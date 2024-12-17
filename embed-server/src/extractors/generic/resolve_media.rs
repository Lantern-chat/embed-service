use super::*;

pub async fn resolve_images(
    state: &ServiceState,
    site: &Option<Arc<Site>>,
    embed: &mut EmbedV1,
) -> Result<(), Error> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let f = FuturesUnordered::new();

    for media in &mut embed.imgs {
        f.push(resolve_media(state, site, media, false));
    }

    if let Some(ref mut media) = embed.thumb {
        f.push(resolve_media(state, site, &mut *media, false));
    }

    // assert this is html
    if let Some(ref mut media) = embed.obj {
        f.push(resolve_media(state, site, &mut *media, true));
    }

    if let Some(ref mut footer) = embed.footer {
        if let Some(ref mut media) = footer.icon {
            f.push(resolve_media(state, site, &mut *media, false));
        }
    }

    if let Some(ref mut author) = embed.author {
        if let Some(ref mut media) = author.icon {
            f.push(resolve_media(state, site, &mut *media, false));
        }
    }

    for field in &mut embed.fields {
        if let Some(ref mut media) = field.img {
            f.push(resolve_media(state, site, &mut *media, true));
        }
    }

    let _ = f.count().await;

    Ok(())
}

pub async fn resolve_media(
    state: &ServiceState,
    site: &Option<Arc<Site>>,
    media: &mut EmbedMedia,
    head: bool,
) -> Result<(), Error> {
    // already has dimensions
    if !head && !matches!((media.width, media.height), (None, None)) {
        return Ok(());
    }

    // TODO: Remove when relative paths are handled
    if media.url.starts_with('.') {
        return Ok(());
    }

    let mut resp = retry_request(2, || {
        let mut req = state.client.request(if head { Method::HEAD } else { Method::GET }, &*media.url);

        if let Some(ref site) = site {
            req = site.add_headers(&state.config, req);
        }

        req
    })
    .await?;

    if let Some(mime) = resp.headers().get("content-type").and_then(|h| h.to_str().ok()) {
        media.mime = Some(mime.into());

        if !head && mime.starts_with("image") {
            // half the max
            let max = state.config.parsed.limits.max_media_size / 2;
            let mut bytes = Vec::with_capacity(max.min(512));

            if let Ok(_) = read_bytes(&mut resp, &mut bytes, max).await {
                if let Ok(image_size) = imagesize::blob_size(&bytes) {
                    media.width = Some(image_size.width as _);
                    media.height = Some(image_size.height as _);
                }
            }
        }
    }

    Ok(())
}
