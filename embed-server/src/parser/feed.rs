use feed_rs::model::{Feed, Image};

use embed::*;

pub fn feed_into_embed(embed: &mut EmbedV1, feed: Feed) -> u64 {
    embed.title = feed.title.map(|t| t.content.into());
    embed.description = feed.description.map(|t| t.content.into());

    if let Some(logo) = feed.logo {
        image_to_media(embed.provider.icon.get_or_insert_with(Default::default), logo);
    }

    if let Some(icon) = feed.icon {
        image_to_media(embed.thumb.get_or_insert_with(Default::default), icon);
    }

    if let Some(ref rating) = feed.rating {
        // TODO: I don't know exactly what this field can contain
        if crate::parser::regexes::ADULT_RATING.is_match(rating.value.as_bytes()) {
            embed.flags |= EmbedFlags::ADULT;
        }
    }

    60 * feed.ttl.unwrap_or(15) as u64
}

fn image_to_media(media: &mut EmbedMedia, image: Image) {
    media.url = image.uri.into();
    media.description = image.title.or(image.description).map(Into::into);
    media.width = image.width.map(|x| x as i32);
    media.height = image.height.map(|x| x as i32);
}
