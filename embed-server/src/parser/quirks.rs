use embed::*;

use url::Url;

use super::StringHelpers;

pub fn resolve_relative(base_url: &Url, embed: &mut EmbedV1) {
    embed.visit_media(|media| {
        // assume these are well-formed
        if media.url.starts_with("https://") || media.url.starts_with("http://") {
            return;
        }

        if media.url.starts_with('.') {
            // TODO
        }

        let old = media.url.as_str();

        let new_url = Url::parse(&'media_url: {
            let mut url = base_url.origin().ascii_serialization();

            // I've seen this before, where "https://" is replaced with "undefined//"
            for prefix in ["undefined//", "//"] {
                if let Some(old) = old.strip_prefix(prefix) {
                    base_url.scheme().clone_into(&mut url);

                    url += "//";
                    url += old;
                    break 'media_url url;
                }
            }

            if !old.starts_with('/') {
                url += "/";
            }

            url += old;
            url
        });

        media.url = match new_url {
            Ok(url) => url.as_str().into(),
            Err(_) => Default::default(),
        };
    });
}

pub fn fix_embed(embed: &mut EmbedV1) {
    // get rid of invalid images introduced through bad embeds
    {
        if let Some(ref img) = embed.img {
            if let Some(ref mime) = img.mime {
                if !mime.starts_with("image") {
                    embed.img = None;
                }
            }
        }

        if let Some(ref obj) = embed.obj {
            if let Some(ref mime) = obj.mime {
                if !mime.starts_with("text/html") {
                    embed.obj = None;
                }
            }
        }

        for field in &mut embed.fields {
            if let Some(ref img) = field.img {
                if let Some(ref mime) = img.mime {
                    if !mime.starts_with("image") {
                        field.img = None;
                    }
                }
            }
        }
    }

    // redundant canonical
    match (&embed.canonical, &embed.url) {
        (Some(canonical), Some(url)) if canonical == url => {
            embed.canonical = None;
        }
        _ => {}
    }

    // redundant description
    match (&embed.title, &embed.description) {
        (Some(title), Some(description)) if title == description => {
            embed.description = None;
        }
        _ => {}
    }

    // redundant thumbnail
    match (&embed.img, &embed.thumb) {
        (Some(img), Some(thumb)) if thumb.url == img.url => {
            embed.thumb = None;
        }
        _ => {}
    }

    // remove empty fields
    embed.fields.retain(|f| !EmbedField::is_empty(f));

    if let Some(ref img) = embed.img {
        match (img.width, img.height) {
            // if there is a tiny main image, relegate it down to a thumbnail
            (Some(w), Some(h)) if w <= 320 && h <= 320 => {
                embed.thumb = std::mem::take(&mut embed.img);

                if embed.ty == EmbedType::Img {
                    embed.ty = EmbedType::Link;
                }
            }
            _ => {}
        }
    }

    // Avoid alt-text that's the same as the description
    if embed.description.is_some() {
        // NOTE: SmolStr uses an Arc internally, so cloning is cheap
        let desc = embed.description.clone();

        embed.visit_media(|media| {
            if media.description == desc {
                media.description = None;
            }
        });
    }

    embed.visit_full_media(EmbedMedia::normalize);

    embed.visit_media(|media| {
        media.description.trim_text(512);
    });

    embed.title.trim_text(1024);
    embed.description.trim_text(2048);
    embed.provider.name.trim_text(196);

    if let Some(ref mut author) = embed.author {
        author.name.trim_text(196);
    }

    super::embed::determine_embed_type(embed);
}
