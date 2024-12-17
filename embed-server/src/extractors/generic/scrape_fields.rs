use super::*;

pub fn scrape_fields(html: &str, embed: &mut EmbedV1, fields: &SiteFieldSelectors) {
    let doc = scraper::Html::parse_document(html);

    macro_rules! extract {
        ($field:ident) => {
            match fields.$field {
                Some(ref selector) => selector.extract(&doc),
                None => None,
            }
        };
    }

    if embed.title.is_none() {
        if let Some(title) = extract!(title) {
            embed.title = Some(title);
        }
    }

    if embed.description.is_none() {
        if let Some(description) = extract!(description) {
            embed.description = Some(description);
        }
    }

    if embed.imgs.is_empty() {
        if let Some(image_url) = extract!(image_url) {
            let mut media = Box::<EmbedMedia>::default().with_url(image_url);

            if let Some(image_alt) = extract!(image_alt) {
                media.description = Some(image_alt);
            }

            if let Some(image_width) = extract!(image_width) {
                media.width = image_width.parse().ok();
            }

            if let Some(image_height) = extract!(image_height) {
                media.height = image_height.parse().ok();
            }

            embed.imgs.push(*media);
        }
    } else if let Some(img) = embed.imgs.first_mut() {
        if img.description.is_none() {
            if let Some(image_alt) = extract!(image_alt) {
                img.description = Some(image_alt);
            }
        }

        if img.width.is_none() {
            if let Some(image_width) = extract!(image_width) {
                img.width = image_width.parse().ok();
            }
        }

        if img.height.is_none() {
            if let Some(image_height) = extract!(image_height) {
                img.height = image_height.parse().ok();
            }
        }
    }

    if embed.author.is_none() {
        if let Some(author_name) = extract!(author_name) {
            let mut author = EmbedAuthor::default();

            author.name = author_name.as_str().into();

            if let Some(author_url) = extract!(author_url) {
                author.url = Some(author_url);
            }

            if let Some(author_icon) = extract!(author_icon) {
                let mut media = Box::<EmbedMedia>::default().with_url(author_icon);

                if let Some(author_icon_alt) = extract!(author_icon_alt) {
                    media.description = Some(author_icon_alt);
                }

                if let Some(author_icon_width) = extract!(author_icon_width) {
                    media.width = author_icon_width.parse().ok();
                }

                if let Some(author_icon_height) = extract!(author_icon_height) {
                    media.height = author_icon_height.parse().ok();
                }

                author.icon = Some(media);
            }

            embed.author = Some(author);
        }
    } else if let Some(author) = embed.author.as_mut() {
        if author.url.is_none() {
            if let Some(author_url) = extract!(author_url) {
                author.url = Some(author_url);
            }
        }

        if author.icon.is_none() {
            if let Some(author_icon) = extract!(author_icon) {
                let mut media = Box::<EmbedMedia>::default().with_url(author_icon);

                if let Some(author_icon_alt) = extract!(author_icon_alt) {
                    media.description = Some(author_icon_alt);
                }

                if let Some(author_icon_width) = extract!(author_icon_width) {
                    media.width = author_icon_width.parse().ok();
                }

                if let Some(author_icon_height) = extract!(author_icon_height) {
                    media.height = author_icon_height.parse().ok();
                }

                author.icon = Some(media);
            }
        } else if let Some(media) = author.icon.as_mut() {
            if media.description.is_none() {
                if let Some(author_icon_alt) = extract!(author_icon_alt) {
                    media.description = Some(author_icon_alt);
                }
            }

            if media.width.is_none() {
                if let Some(author_icon_width) = extract!(author_icon_width) {
                    media.width = author_icon_width.parse().ok();
                }
            }

            if media.height.is_none() {
                if let Some(author_icon_height) = extract!(author_icon_height) {
                    media.height = author_icon_height.parse().ok();
                }
            }
        }
    }

    if embed.provider.name.is_none() {
        if let Some(provider_name) = extract!(provider_name) {
            embed.provider.name = Some(provider_name.as_str().into());
        }
    }

    if embed.provider.url.is_none() {
        if let Some(provider_url) = extract!(provider_url) {
            embed.provider.url = Some(provider_url);
        }
    }

    if embed.provider.icon.is_none() {
        if let Some(provider_icon) = extract!(provider_icon) {
            let mut media = Box::<EmbedMedia>::default().with_url(provider_icon);

            if let Some(provider_icon_alt) = extract!(provider_icon_alt) {
                media.description = Some(provider_icon_alt);
            }

            if let Some(provider_icon_width) = extract!(provider_icon_width) {
                media.width = provider_icon_width.parse().ok();
            }

            if let Some(provider_icon_height) = extract!(provider_icon_height) {
                media.height = provider_icon_height.parse().ok();
            }

            embed.provider.icon = Some(media);
        }
    } else if let Some(media) = embed.provider.icon.as_mut() {
        if media.description.is_none() {
            if let Some(provider_icon_alt) = extract!(provider_icon_alt) {
                media.description = Some(provider_icon_alt);
            }
        }

        if media.width.is_none() {
            if let Some(provider_icon_width) = extract!(provider_icon_width) {
                media.width = provider_icon_width.parse().ok();
            }
        }

        if media.height.is_none() {
            if let Some(provider_icon_height) = extract!(provider_icon_height) {
                media.height = provider_icon_height.parse().ok();
            }
        }
    }
}
