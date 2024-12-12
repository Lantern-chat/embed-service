use hashbrown::HashMap;
use std::sync::LazyLock;

use super::prelude::*;

pub struct E621ExtractorFactory;

impl ExtractorFactory for E621ExtractorFactory {
    fn create(&self, config: &Config) -> Result<Option<Box<dyn Extractor>>, ConfigError> {
        let Some(extractor) =
            config.parsed.extractors.get("e621").or_else(|| config.parsed.extractors.get("e926"))
        else {
            return Ok(None);
        };

        let Some(login) = extractor.get("login").cloned() else {
            return Err(ConfigError::MissingExtractorField("e621.login"));
        };

        let Some(api_key) = extractor.get("api_key").cloned() else {
            return Err(ConfigError::MissingExtractorField("e621.api_key"));
        };

        Ok(Some(Box::new(E621Extractor { login, api_key })))
    }
}

#[derive(Debug)]
pub struct E621Extractor {
    pub login: String,
    pub api_key: String,
}

#[async_trait::async_trait]
impl Extractor for E621Extractor {
    fn matches(&self, url: &Url) -> bool {
        // TODO: Support more than /posts/
        matches!(url.domain(), Some("e621.net" | "e926.net")) && url.path().starts_with("/posts/")
    }

    #[instrument(skip_all)]
    async fn extract(
        &self,
        state: Arc<ServiceState>,
        url: Url,
        params: Params,
    ) -> Result<EmbedWithExpire, Error> {
        let Some(mut segments) = url.path_segments() else {
            return Err(Error::Failure(StatusCode::BAD_REQUEST));
        };

        let which = match url.domain() {
            Some("e621.net") => Which::E621,
            Some("e926.net") => Which::E926,
            _ => unreachable!(),
        };

        let section = match segments.next() {
            Some("posts") => Section::Posts,
            Some("users") => Section::Users,
            Some("artists") => Section::Artists,
            Some("pool") => Section::Pool,
            _ => return Err(Error::Failure(StatusCode::NOT_FOUND)),
        };

        let req = match (section, segments.next()) {
            (Section::Posts, Some(id)) => fetch_single_id(self, state, &url, id, which).boxed(),
            _ => return Err(Error::Failure(StatusCode::NOT_FOUND)),
        };

        req.await
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Which {
    E621,
    E926,
}

pub enum Section {
    Posts,
    Users,
    Artists,
    Pool,
}

pub mod models;
use models::*;

#[allow(clippy::field_reassign_with_default)]
#[instrument(skip(extractor, state))]
async fn fetch_single_id(
    extractor: &E621Extractor,
    state: Arc<ServiceState>,
    url: &Url,
    id: &str,
    which: Which,
) -> Result<EmbedWithExpire, Error> {
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::Failure(StatusCode::NOT_FOUND));
    }

    let resp = state
        .client
        .get(format!(
            "https://e621.net/posts.json?login={}&api_key={}&limit=1&tags=id:{id}",
            extractor.login, extractor.api_key
        ))
        .send()
        .await?;

    let E621Result::Success(SinglePost::Found { posts: [mut post] }) = resp.json().await? else {
        return Err(Error::Failure(StatusCode::NOT_FOUND));
    };

    // e926 is specifically to avoid explicit content, but the API ignores that
    // so filter it here
    if post.rating != Rating::Safe && which == Which::E926 {
        return Err(Error::Failure(StatusCode::NOT_FOUND));
    }

    let mut file = &post.file;

    match post.sample {
        // large file, revert to sample
        Some(ref sample) if file.height > 2048 || file.width > 2048 => {
            file = &sample.file;
        }
        _ => {}
    }

    let Some(ref file_url) = file.url else {
        return Err(Error::Failure(StatusCode::NOT_FOUND));
    };

    // NOTE: The order of field initialization is such that it avoids heavy work
    // if dumb/simple things fail early.
    let mut embed = EmbedV1::default();

    // questionable can still contain nudity, so safe is the only one that's truly safe
    if post.rating != Rating::Safe {
        embed.flags |= EmbedFlags::ADULT;
    }

    // NOTE: I don't enjoy that I have to list these, but they
    // need to be marked as graphic.
    static GRAPHIC_TAGS: LazyLock<TagChecker> =
        LazyLock::new(|| TagChecker::new(["gore", "snuff", "necrophilia"]));

    if post.tags.general.iter().any(|tag| GRAPHIC_TAGS.contains(tag)) {
        embed.flags |= EmbedFlags::GRAPHIC;
    }

    let mut main_embed =
        Box::<EmbedMedia>::default().with_url(file_url).with_dims(file.width as _, file.height as _);

    if let Some(ext) = main_embed.url.split('.').last() {
        let mime = mime_guess::from_ext(ext).first();

        main_embed.mime = mime.as_ref().map(|m| m.to_smolstr());

        match mime.as_ref().map(|m| m.type_().as_str()) {
            Some("image") => embed.imgs.push(*main_embed),
            Some("video") => embed.video = Some(main_embed),
            Some("audio") => embed.audio = Some(main_embed),
            _ if post.preview.is_some() => {
                if let Some(ref preview) = post.preview {
                    if let Some(ref url) = preview.url {
                        main_embed.url = url.into();
                        main_embed.width = Some(preview.width as _);
                        main_embed.height = Some(preview.height as _);
                    }
                }
            }
            _ => {}
        }
    }

    if embed.imgs.is_empty() && embed.video.is_none() && embed.audio.is_none() {
        return Err(Error::Failure(StatusCode::UNSUPPORTED_MEDIA_TYPE));
    }

    'vid_alt: {
        let Some(ref mut video) = embed.video else {
            break 'vid_alt;
        };

        let Some(ref mime) = video.mime else {
            break 'vid_alt;
        };

        // mp4 can be played almost universally
        if mime.ends_with("mp4") {
            break 'vid_alt;
        }

        let Some(ref sample) = post.sample else {
            break 'vid_alt;
        };

        let mut alt = None;
        for key in ["original", "1080p", "720p", "480p", "360p", "240p"] {
            alt = sample.alternates.get(key);
            if alt.is_some() {
                break;
            }
        }
        let Some(alt) = alt else {
            break 'vid_alt;
        };

        let Some(Some(url)) = alt.urls.iter().find(|&url| matches!(url, Some(url) if !url.ends_with("webm")))
        else {
            break 'vid_alt;
        };

        let mut alt_media = BasicEmbedMedia::default();

        alt_media.url = url.into();
        alt_media.width = Some(alt.width as _);
        alt_media.height = Some(alt.height as _);

        alt_media.mime =
            url.split('.').last().and_then(|ext| Some(mime_guess::from_ext(ext).first()?.to_smolstr()));

        video.alts.push(alt_media);
    }

    embed.url = Some({
        let mut u = url.origin().ascii_serialization();
        write!(u, "/posts/{id}").unwrap();
        u.into()
    });

    embed.title = Some({
        let mut title = post.generate_title().unwrap();

        title += match which {
            Which::E621 => " - e621",
            Which::E926 => " - e926",
        };

        title.into()
    });

    embed.author = post.generate_author().unwrap().map(|name| EmbedAuthor {
        name: name.into(),
        ..Default::default()
    });

    embed.description = match crate::util::trim_text(&post.description) {
        std::borrow::Cow::Borrowed(_) => Some(post.description),
        std::borrow::Cow::Owned(desc) => Some(desc.into()),
    };

    embed.color = Some(0x00549e);

    embed.provider = match which {
        Which::E621 => E621_PROVIDER.clone(),
        Which::E926 => E926_PROVIDER.clone(),
    };

    // 4-hour expire
    Ok(generic::finalize_embed(state, embed, Some(60 * 60 * 4)))
}

static E621_PROVIDER: Lazy<EmbedProvider> = Lazy::new(|| {
    let mut provider = EmbedProvider::default();
    provider.name = Some(SmolStr::new_inline("e621"));
    provider.url = Some(ThinString::from("https://e621.net"));
    provider.icon = Some(Box::<EmbedMedia>::default().with_url("https://e621.net/apple-touch-icon.png"));
    provider
});

static E926_PROVIDER: Lazy<EmbedProvider> = Lazy::new(|| {
    let mut provider = EmbedProvider::default();
    provider.name = Some(SmolStr::new_inline("e926"));
    provider.url = Some(ThinString::from("https://e926.net"));
    provider.icon = Some(Box::<EmbedMedia>::default().with_url("https://e926.net/apple-touch-icon.png"));
    provider
});
