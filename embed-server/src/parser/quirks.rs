use std::borrow::Cow;

use embed::*;

use smol_str::ToSmolStr;
use url::Url;

use super::StringHelpers;

pub fn resolve_relative_url(base_url: &Url, mut old_url: &str) -> Result<Url, url::ParseError> {
    Url::parse(&'url: {
        // assume these are well-formed
        if old_url.starts_with("https://") || old_url.starts_with("http://") {
            break 'url Cow::Borrowed(old_url);
        }

        let mut url = base_url.origin().ascii_serialization();

        if let Some(ou) = old_url.strip_prefix("./") {
            if let Some((path, _)) = base_url.path().rsplit_once('/') {
                url += path;
            }

            // if base_url as at the root, the above branch won't trigger, but we still
            // need to strip the ./
            old_url = ou;
        }

        // I've seen this before, where "https://" is replaced with "undefined//"
        for prefix in ["undefined//", "//"] {
            if let Some(old) = old_url.strip_prefix(prefix) {
                base_url.scheme().clone_into(&mut url);

                url += "//";
                url += old;
                break 'url Cow::Owned(url);
            }
        }

        if !old_url.starts_with('/') {
            url += "/";
        }

        url += old_url;
        Cow::Owned(url)
    })
}

pub fn resolve_relative(base_url: &Url, embed: &mut EmbedV1) {
    embed.visit_media(|media| {
        media.url = match resolve_relative_url(base_url, &media.url) {
            Ok(url) => url.as_str().into(),
            Err(_) => Default::default(),
        };
    });
}

pub fn fix_embed(embed: &mut EmbedV1) {
    // get rid of invalid images introduced through bad embeds
    {
        embed.imgs.retain(|img| match img.mime {
            Some(ref mime) => mime.starts_with("image"),
            None => false,
        });

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

    if let Some(ref thumb) = embed.thumb {
        // redundant thumbnail
        if embed.imgs.iter().any(|img| img.url == thumb.url) {
            embed.thumb = None;
        }
    }

    // remove empty fields
    embed.fields.retain(|f| !EmbedField::is_empty(f));

    match embed.imgs.first() {
        Some(img) if embed.imgs.len() == 1 => {
            match (img.width, img.height) {
                // if there is a tiny main image, relegate it down to a thumbnail
                (Some(w), Some(h)) if w <= 320 && h <= 320 => {
                    embed.thumb = embed.imgs.pop().map(Box::new);

                    if embed.ty == EmbedType::Img {
                        embed.ty = EmbedType::Link;
                    }
                }
                _ => {}
            }
        }
        None if embed.ty == EmbedType::Img => {
            embed.ty = EmbedType::Link;
        }
        _ => {}
    }

    // Avoid alt-text that's the same as the description
    if let Some(ref desc) = embed.description {
        // cloning the description just to allow another mutable borrow of embed
        // is very wasteful, so we're going to cheat.
        //
        // SAFETY: This ref doesn't outlive this block, and we can guarantee that
        // the embed.description is never modified.
        let never_do_this = unsafe { core::mem::transmute::<&str, &'static str>(desc.as_str()) };

        embed.visit_media(|media| {
            if matches!(media.description, Some(ref d) if d == never_do_this) {
                media.description = None;
            }
        });
    }

    embed.visit_full_media(EmbedMedia::normalize);

    embed.visit_media(|media| {
        media.description.trim_text(512);

        if media.mime.is_none() {
            if let Some((_, ext)) = media.url.rsplit_once('.') {
                media.mime = mime_guess::from_ext(ext).first().map(|m| m.to_smolstr());
            }
        }
    });

    embed.title.trim_text(1024);
    embed.description.trim_text(2048);
    embed.provider.name.trim_text(196);

    if let Some(ref mut author) = embed.author {
        author.name.trim_text(196);
    }

    super::embed::determine_embed_type(embed);
}

#[cfg(test)]
mod tests {
    use super::{resolve_relative_url, Url};

    #[test]
    fn test_resolve_relative_url() {
        let base = Url::parse("https://example.com/test/page.php").unwrap();

        assert_eq!(
            resolve_relative_url(&base, "https://example.com/test/page.php"),
            Ok(Url::parse("https://example.com/test/page.php").unwrap())
        );

        assert_eq!(
            resolve_relative_url(&base, "test/page.php"),
            Ok(Url::parse("https://example.com/test/page.php").unwrap())
        );

        assert_eq!(
            resolve_relative_url(&base, "/test/page.php"),
            Ok(Url::parse("https://example.com/test/page.php").unwrap())
        );

        assert_eq!(
            resolve_relative_url(&base, "page.php"),
            Ok(Url::parse("https://example.com/page.php").unwrap())
        );

        assert_eq!(
            resolve_relative_url(&base, "./page.php"),
            Ok(Url::parse("https://example.com/test/page.php").unwrap())
        );

        assert_eq!(
            resolve_relative_url(&base, "./test/page.php"),
            Ok(Url::parse("https://example.com/test/test/page.php").unwrap())
        );

        let base = Url::parse("https://example.com/test/").unwrap();

        assert_eq!(
            resolve_relative_url(&base, "./page.php"),
            Ok(Url::parse("https://example.com/test/page.php").unwrap())
        );
    }
}
