use super::*;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyProfile {
    pub did: String,

    pub handle: SmolStr,

    #[serde(default)]
    pub display_name: SmolStr,

    #[serde(default)]
    pub avatar: ThinString,

    #[serde(default)]
    pub description: ThinString,

    #[serde(default)]
    pub banner: ThinString,

    #[serde(default)]
    pub labels: Vec<BskyLabel>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "$type")]
pub enum BskyRecord {
    #[serde(rename = "app.bsky.feed.post", rename_all = "camelCase")]
    Post {
        created_at: Timestamp,
        text: ThinString,

        /// Only present with nested embeds
        #[serde(default)]
        author: Option<BskyProfile>,
    },

    #[serde(rename = "app.bsky.embed.record#viewRecord", alias = "app.bsky.embed.record")]
    Record {
        value: Box<BskyRecord>,

        #[serde(default)]
        embeds: Vec<BskyEmbed>,

        author: BskyProfile,
    },

    #[serde(other)]
    Unknown,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyAspectRatio {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyEmbedImage {
    #[serde(default)]
    pub thumb: ThinString,

    #[serde(default)]
    pub fullsize: ThinString,

    #[serde(default)]
    pub alt: ThinString,

    pub aspect_ratio: BskyAspectRatio,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyEmbedExternal {
    pub uri: ThinString,
    pub title: ThinString,

    #[serde(default)]
    pub description: ThinString,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyEmbedVideo {
    #[serde(default)]
    pub thumbnail: ThinString,

    pub playlist: ThinString,

    pub aspect_ratio: BskyAspectRatio,
}

#[derive(Debug, serde::Deserialize)]
pub struct NestedRecord {
    pub record: BskyRecord,
}

// NOTE: I don't know how consistent these tags are
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "$type")]
pub enum BskyEmbed {
    #[serde(rename = "app.bsky.embed.images#view", alias = "app.bsky.embed.images")]
    Images { images: Vec<BskyEmbedImage> },

    #[serde(rename = "app.bsky.embed.external#view", alias = "app.bsky.embed.external")]
    External { external: BskyEmbedExternal },

    #[serde(rename = "app.bsky.embed.video#view", alias = "app.bsky.embed.video")]
    Video {
        #[serde(flatten)] // why is this one unique?
        video: BskyEmbedVideo,
    },

    #[serde(
        rename = "app.bsky.embed.recordWithMedia#view",
        alias = "app.bsky.embed.recordWithMedia"
    )]
    RecordWithMedia {
        media: Box<BskyEmbed>,
        record: Box<NestedRecord>,
    },

    #[serde(rename = "app.bsky.embed.record#view", alias = "app.bsky.embed.record")]
    Record { record: Box<BskyRecord> },

    #[serde(other)]
    Unknown,
}

impl BskyEmbed {
    pub fn embed(self) -> BskyEmbed {
        match self {
            BskyEmbed::RecordWithMedia { media, .. } => *media,
            _ => self,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyPost {
    pub record: BskyRecord,

    pub embed: BskyEmbed,

    pub reply_count: u32,
    pub repost_count: u32,
    pub like_count: u32,
    pub quote_count: u32,

    #[serde(default)]
    pub labels: Vec<BskyLabel>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BskyPosts {
    #[serde(default)]
    pub posts: Vec<BskyPost>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BskyLabel {
    pub val: SmolStr,

    #[serde(default)]
    pub neg: bool,
}

impl BskyLabel {
    pub fn flags(&self) -> EmbedFlags {
        // there are certain undocumented labels that contain these strings
        // and should mean the same thing. "sexual-figurative" for example.
        // So by checking for containing, we can catch more of them.
        for adult_label in ["porn", "sexual", "nudity", "adult", "explicit"].iter() {
            if self.val.contains(adult_label) {
                return EmbedFlags::ADULT;
            }
        }

        if self.val.contains("spoiler") {
            return EmbedFlags::SPOILER;
        }

        if self.val.contains("graphic-media") {
            return EmbedFlags::ADULT | EmbedFlags::SPOILER;
        }

        EmbedFlags::empty()
    }

    pub fn aggregate_flags(labels: &[BskyLabel]) -> EmbedFlags {
        let start = (EmbedFlags::empty(), EmbedFlags::empty());

        // labels have the `neg` field that removes the previous label
        let (a, b) = labels.iter().fold(start, |(acc, prev), label| {
            if label.neg {
                (acc, label.flags())
            } else {
                (acc | prev, label.flags())
            }
        });

        a | b
    }
}

pub fn write_footer<W>(
    w: &mut W,
    ts: Option<Timestamp>,
    like_count: u32,
    reply_count: u32,
    repost_count: u32,
    quote_count: u32,
) -> core::fmt::Result
where
    W: core::fmt::Write,
{
    use core::fmt::Write;

    // TODO: Friendly formatting
    if let Some(ts) = ts {
        write!(w, "{ts} - ")?;
    }

    let symbols = [
        (like_count, "❤️"),
        (reply_count, "💬"),
        (repost_count, "🔁"),
        (quote_count, "🔖"),
    ];

    let mut prev = false;

    for (count, symbol) in symbols.iter() {
        if *count > 0 {
            if prev {
                w.write_str(" | ");
            }

            write!(w, "{symbol} {count}")?;
            prev = true;
        }
    }

    Ok(())
}

pub fn append_description<W>(w: &mut W, display_name: &str, handle: &str, text: &str) -> core::fmt::Result
where
    W: core::fmt::Write,
{
    use core::fmt::Write;

    let author_name = if display_name.is_empty() { handle } else { display_name };

    // block-quote the author name and post text
    writeln!(w, "\n\n> **@{} ({})**", handle, author_name.trim()).unwrap();

    for line in text.lines() {
        write!(w, "\n> {}", line.trim()).unwrap();
    }

    Ok(())
}
