use super::prelude::*;

pub struct ImgurExtractorFactory;

#[derive(Debug)]
pub struct ImgurExtractor {
    pub client_id: HeaderValue,
}

impl ExtractorFactory for ImgurExtractorFactory {
    fn create(&self, config: &Config) -> Result<Option<Box<dyn Extractor>>, ConfigError> {
        let Some(extractor) = config.parsed.extractors.get("imgur") else {
            return Ok(None);
        };

        let Some(client_id) = extractor.get("client_id") else {
            return Err(ConfigError::MissingExtractorField("imgur.client_id"));
        };

        let Ok(client_id) = HeaderValue::try_from(format!("Client-ID {client_id}")) else {
            return Err(ConfigError::InvalidExtractorField("imgur.client_id"));
        };

        Ok(Some(Box::new(ImgurExtractor { client_id })))
    }
}

// These are just some known path segments that can't be embedded
const BAD_PATHS: &[&str] = &[
    "user", "upload", "signin", "emerald", "vidgif", "memegen", "apps", "search",
];

#[async_trait::async_trait]
impl Extractor for ImgurExtractor {
    fn matches(&self, url: &Url) -> bool {
        if !matches!(url.domain(), Some("imgur.com" | "i.imgur.com")) {
            return false;
        }

        let Some(mut segments) = url.path_segments() else {
            return false;
        };

        let potential_image_id = match segments.next() {
            Some("gallery" | "a") => match segments.next() {
                Some(potential_image_id) => potential_image_id,
                None => return false,
            },
            Some(potential_image_id) if !BAD_PATHS.contains(&potential_image_id) => potential_image_id,
            _ => return false,
        };

        // strip file extension if present
        let Some(potential_image_id) = potential_image_id.split('.').next() else {
            return false;
        };

        // urls contain post titles now, so we have to support that
        potential_image_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    #[instrument(skip_all)]
    async fn extract(
        &self,
        state: Arc<ServiceState>,
        url: Url,
        params: Params,
    ) -> Result<EmbedWithExpire, Error> {
        let Some(mut segments) = url.path_segments() else {
            return Err(Error::Failure(StatusCode::NOT_FOUND));
        };

        let (id, api) = match segments.next() {
            Some(seg @ ("gallery" | "a")) => match segments.next() {
                Some(id) => (id, if seg == "a" { "album" } else { "gallery/album" }),
                None => unreachable!(),
            },
            Some(id) => (id, "image"),
            _ => unreachable!(),
        };

        // strip file extension if present
        let Some(id) = id.split('.').next() else {
            return Err(Error::Failure(StatusCode::NOT_FOUND));
        };

        // trim any post titles from the URL
        let Some(id) = id.split('-').last() else {
            return Err(Error::Failure(StatusCode::NOT_FOUND));
        };

        let resp = state
            .client
            .get(format!("https://api.imgur.com/3/{api}/{id}"))
            .header(HeaderName::from_static("authorization"), &self.client_id)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::Failure(resp.status()));
        }

        let resp = resp.json().await?;

        let ImgurResult::Success {
            data: Some(mut data), ..
        } = resp
        else {
            return Err(Error::Failure(StatusCode::NOT_FOUND));
        };

        let mut embed = EmbedV1::default();

        #[rustfmt::skip]
        let images: &mut [ImgurImageData] = match data.kind {
            | ImgurDataKind::Gallery { ref mut images, .. }
            | ImgurDataKind::Album { ref mut images, .. } => match data.cover {
                Some(ref cover) => match images.iter_mut().find(|img| img.id == *cover) {
                    Some(image) => core::slice::from_mut(image),
                    None => images.as_mut_slice(),
                },
                None => images.as_mut_slice(),
            },
            ImgurDataKind::Image { ref mut image } => {
                core::slice::from_mut(image)
            }
        };

        if images.is_empty() {
            return Err(Error::Failure(StatusCode::NOT_FOUND));
        }

        let mut num_media = 0;

        for image in images.iter_mut().take(state.config.parsed.limits.max_images) {
            let mut media = EmbedMedia::default();

            // add ?noredirect to imgur links because they're annoying
            media.url = add_noredirect(std::mem::take(&mut image.link)).into();

            media.width = image.width;
            media.height = image.height;

            match image.mime.take() {
                Some(mime) if mime.contains('/') => media.mime = Some(mime),
                _ => {}
            }

            match media.mime {
                Some(ref mime) if mime.starts_with("video") => {
                    embed.imgs.clear();

                    match image.mp4.take() {
                        Some(mp4) if mime.ends_with("webm") => {
                            let mut alt = media.media.clone();
                            alt.mime = Some(SmolStr::new_inline("video/mp4"));
                            alt.url = add_noredirect(mp4).into();
                            media.alts.push(alt);
                        }
                        _ => {}
                    }

                    embed.video = Some(Box::new(media));

                    num_media = 1;

                    break;
                }
                _ => embed.imgs.push(media),
            }

            num_media += 1;
        }

        static IMGUR_PROVIDER: LazyLock<EmbedProvider> = LazyLock::new(|| {
            let mut provider = EmbedProvider::default();

            provider.name = Some(SmolStr::new_inline("imgur"));
            provider.url = Some(ThinString::from("https://imgur.com"));
            provider.icon =
                Some(Box::<EmbedMedia>::default().with_url("https://s.imgur.com/images/favicon.png"));

            provider
        });

        embed.provider = IMGUR_PROVIDER.clone();

        if match (data.nsfw, data.ad_config) {
            (Some(true), _) => true,
            (_, Some(ref ad_config)) => ad_config.nsfw_score > 0.75,
            _ => false,
        } {
            embed.flags |= EmbedFlags::ADULT;
        }

        embed.url = Some({
            let mut origin = url.origin().ascii_serialization();
            origin += url.path();
            origin.into()
        });

        embed.title = data.title;
        embed.description = data.description;

        embed.color = Some(0x85bf25);

        let remaining_images = data.images_count - num_media;

        if remaining_images > 0 {
            embed.footer = Some(EmbedFooter {
                text: format_thin_string!(
                    "and {remaining_images} more {}",
                    match remaining_images {
                        1 => "file",
                        _ => "files",
                    }
                ),
                icon: None,
            });
        }

        // 4-hour expire
        Ok(generic::finalize_embed(state, embed, Some(60 * 60 * 4)))
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ImgurResult {
    Success {
        success: monostate::MustBe!(true),

        #[serde(default)]
        data: Option<ImgurData>,
    },
    Failure {},
}

#[derive(Debug, serde::Deserialize)]
pub struct ImgurData {
    #[serde(default)]
    pub ad_config: Option<ImgurAdConfig>,

    #[serde(default)]
    pub images_count: usize,

    #[serde(flatten)]
    pub kind: ImgurDataKind,

    #[serde(default)]
    pub cover: Option<SmolStr>,

    #[serde(default)]
    pub title: Option<ThinString>,

    #[serde(default)]
    pub description: Option<ThinString>,

    #[serde(default)]
    pub nsfw: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ImgurDataKind {
    Gallery {
        is_gallery: monostate::MustBe!(true),

        #[serde(default)]
        images: Vec<ImgurImageData>,
    },
    Album {
        is_album: monostate::MustBe!(true),

        #[serde(default)]
        images: Vec<ImgurImageData>,
    },
    Image {
        #[serde(flatten)]
        image: ImgurImageData,
    },
}

#[derive(Debug, serde::Deserialize)]
pub struct ImgurImageData {
    pub id: SmolStr,

    #[serde(default, rename = "type")]
    pub mime: Option<SmolStr>,

    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,

    pub link: String,

    #[serde(default)]
    pub mp4: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ImgurAdConfig {
    #[serde(default)]
    pub nsfw_score: f32,
}

fn add_noredirect(mut url: String) -> String {
    if !url.ends_with("?noredirect") {
        url += "?noredirect";
    }
    url
}
