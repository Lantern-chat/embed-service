use hashbrown::HashMap;

use super::prelude::*;

#[derive(Debug)]
pub struct GenericExtractor;

impl ExtractorFactory for GenericExtractor {
    fn create(&self, _config: &Config) -> Result<Option<Box<dyn Extractor>>, ConfigError> {
        Ok(Some(Box::new(GenericExtractor)))
    }
}

/// Extracts an embed from a URL using generic/standard attributes
pub async fn extract(
    state: Arc<ServiceState>,
    url: url::Url,
    params: Params,
) -> Result<EmbedWithExpire, Error> {
    let RawGenericExtraction {
        state,
        embed,
        max_age,
    } = extract_raw(state, url, params).await?;

    Ok(finalize_embed(state, embed, max_age))
}

pub struct RawGenericExtraction {
    pub state: Arc<ServiceState>,
    pub embed: EmbedV1,
    pub max_age: Option<u64>,
}

/// Extracts an embed from a URL using generic/standard attributes,
/// but doesn't finalize it
pub async fn extract_raw(
    state: Arc<ServiceState>,
    url: url::Url,
    params: Params,
) -> Result<RawGenericExtraction, Error> {
    if !url.scheme().starts_with("http") {
        return Err(Error::InvalidUrl);
    }

    let site = url.domain().and_then(|domain| state.config.find_site(domain));

    let mut resp = retry_request(2, || {
        let mut req = state.client.get(url.as_str());

        if let Some(ref site) = site {
            req = site.add_headers(&state.config, req);
        }

        if let Some(ref lang) = params.lang {
            req = req.header(
                HeaderName::from_static("accept-language"),
                format!("{lang};q=0.5"),
            );
        }

        req
    })
    .await?;

    if !resp.status().is_success() {
        return Err(Error::Failure(resp.status()));
    }

    let mut embed = EmbedV1::default();
    let mut oembed: Option<OEmbed> = None;

    // seconds until embed expires
    let mut max_age = None;

    if let Some(rating) = resp.headers().get(HeaderName::from_static("rating")) {
        if crate::parser::patterns::contains_adult_rating(rating.as_bytes()) {
            embed.flags |= EmbedFlags::ADULT;
        }
    }

    let links = resp
        .headers()
        .get("link")
        .and_then(|h| h.to_str().ok())
        .map(crate::parser::oembed::parse_link_header);

    embed.url = Some(url.as_str().into());

    if let Some(link) = links.as_ref().and_then(|l| l.first()) {
        if let Ok(o) = fetch_oembed(&state, link, url.domain()).await {
            oembed = o;
        }
    }

    drop(links);

    if let Some(mime) = resp.headers().get("content-type").and_then(|h| h.to_str().ok()) {
        let Some(mime) = mime.split(';').next() else {
            return Err(Error::InvalidMimeType);
        };

        if mime == "text/html" {
            let max = state.config.parsed.limits.max_html_size;
            let mut html = Vec::with_capacity(max.min(512));

            let body = read_body(&mut resp, &mut html, max).await?;

            std::fs::write("test.html", body).unwrap();

            if let Some(headers) = crate::parser::html::parse_meta(body) {
                let extra = crate::parser::embed::parse_meta_to_embed(&mut embed, &headers);

                match extra.link {
                    Some(link) if oembed.is_none() => {
                        if let Ok(o) = fetch_oembed(&state, &link, url.domain()).await {
                            oembed = o;
                        }
                    }
                    _ => {}
                }

                max_age = extra.max_age;
            }

            match site {
                Some(ref site) if !site.fields.is_empty() => {
                    scrape_fields::scrape_fields(body, &mut embed, &site.fields)
                }
                _ => {}
            }

            drop(html); // ensure it lives long enough
        } else if matches!(
            mime,
            "application/rss+xml" | "application/feed+json" | "application/atom+xml" | "application/xml"
        ) {
            let max = state.config.parsed.limits.max_xml_size;
            let mut body = Vec::with_capacity(max.min(512));

            if let Ok(_) = read_bytes(&mut resp, &mut body, max).await {
                // TODO: Maybe set the timestamp parser to use iso8601_timestamp
                let parser = feed_rs::parser::Builder::new().base_uri(Some(url.as_str())).build();

                if let Ok(feed) = parser.parse(&*body) {
                    max_age = Some(crate::parser::feed::feed_into_embed(&mut embed, feed));
                }
            }

            drop(body);
        } else {
            let mut media = Box::<EmbedMedia>::default();
            media.url = url.as_str().into();
            media.mime = Some(mime.into());

            match mime.get(0..5) {
                Some("image") => {
                    let max = state.config.parsed.limits.max_media_size;
                    let mut bytes = Vec::with_capacity(max.min(512));

                    if let Ok(_) = read_bytes(&mut resp, &mut bytes, max).await {
                        if let Ok(image_size) = imagesize::blob_size(&bytes) {
                            media.width = Some(image_size.width as _);
                            media.height = Some(image_size.height as _);
                        }
                    }

                    embed.ty = EmbedType::Img;
                    embed.imgs.push(*media);
                }
                Some("video") => {
                    embed.ty = EmbedType::Vid;
                    embed.video = Some(media);
                }
                Some("audio") => {
                    embed.ty = EmbedType::Audio;
                    embed.audio = Some(media);
                }
                _ => {}
            }
        }
    }

    if let Some(oembed) = oembed {
        let extra = crate::parser::embed::parse_oembed_to_embed(&mut embed, oembed);

        max_age = extra.max_age;
    }

    crate::parser::quirks::resolve_relative(&url, &mut embed);

    if state.config.parsed.resolve_media {
        resolve_media::resolve_images(&state, &site, &mut embed).await?;
    }

    if let Some(domain) = url.domain() {
        if !state.config.allow_html(domain).is_match() {
            embed.obj = None;

            if let Some(ref vid) = embed.video {
                if matches!(vid.mime, Some(ref mime) if mime.starts_with("text/html")) {
                    embed.video = None;
                }
            }
        }

        if let Some(site) = site {
            embed.color = site.color.or(embed.color);
        }
    }

    Ok(RawGenericExtraction {
        state,
        embed,
        max_age,
    })
}

#[async_trait::async_trait]
impl Extractor for GenericExtractor {
    fn matches(&self, _: &url::Url) -> bool {
        true
    }

    #[instrument(skip_all)]
    async fn extract(
        &self,
        state: Arc<ServiceState>,
        url: url::Url,
        params: Params,
    ) -> Result<EmbedWithExpire, Error> {
        extract(state, url, params).await
    }
}

pub fn finalize_embed(state: Arc<ServiceState>, mut embed: EmbedV1, max_age: Option<u64>) -> EmbedWithExpire {
    crate::parser::quirks::fix_embed(&mut embed);

    if state.signing_key.is_some() {
        embed.visit_media(|media| {
            media.signature = state.sign(&media.url);
        });
    }

    let expires = {
        embed.ts = Timestamp::now_utc();

        // limit max_age to 1 month, minimum 15 minutes
        embed
            .ts
            .checked_add(Duration::seconds(
                max_age.unwrap_or(60 * 15).clamp(60 * 15, 60 * 60 * 24 * 30) as i64,
            ))
            .unwrap()
    };

    (expires, embed::Embed::V1(embed))
}

pub async fn fetch_oembed(
    state: &ServiceState,
    link: &OEmbedLink<'_>,
    domain: Option<&str>,
) -> Result<Option<OEmbed>, Error> {
    if let Some(domain) = domain {
        if state.config.skip_oembed(domain).is_match() {
            return Ok(None);
        }
    }

    let body = state.client.get(&*link.url).send().await?.bytes().await?;

    Ok(Some(match link.format {
        OEmbedFormat::JSON => json_impl::from_slice(&body)?,
        OEmbedFormat::XML => quick_xml::de::from_reader(&*body)?,
    }))
}

pub async fn read_body<'a>(
    resp: &mut reqwest::Response,
    html: &'a mut Vec<u8>,
    max: usize,
) -> Result<&'a str, Error> {
    while let Some(chunk) = resp.chunk().await? {
        html.extend(&chunk);

        if memchr::memmem::rfind(html, b"</body").is_some() {
            break;
        }

        // Limits of HTML downloaded, assume it's a broken page or DoS attack and don't bother with more
        if html.len() > max {
            break;
        }
    }

    if let Cow::Owned(new_html) = String::from_utf8_lossy(html) {
        *html = new_html.into();
    }

    // SAFETY: Just converted it to lossy utf8, it's fine
    Ok(unsafe { std::str::from_utf8_unchecked(html) })
}

pub async fn read_bytes<'a>(
    resp: &'a mut reqwest::Response,
    bytes: &'a mut Vec<u8>,
    max: usize,
) -> Result<(), Error> {
    while let Some(chunk) = resp.chunk().await? {
        bytes.extend(&chunk);

        if bytes.len() > max {
            break;
        }
    }

    Ok(())
}

pub async fn retry_request<F>(max_attempts: u8, mut make_request: F) -> Result<reqwest::Response, Error>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut req = make_request().send().boxed();
    let mut attempts = 1;

    loop {
        match req.await {
            Ok(resp) => break Ok(resp),
            Err(e) if e.is_timeout() && attempts < max_attempts => {
                attempts += 1;
                req = make_request().send().boxed();
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub mod resolve_media;
pub mod scrape_fields;
pub mod web_manifest;
