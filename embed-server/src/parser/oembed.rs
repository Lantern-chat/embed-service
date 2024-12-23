//use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OEmbedFormat {
    JSON = 1,
    XML = 2,
}

#[derive(Debug, PartialEq)]
pub struct OEmbedLink<'a> {
    pub url: Cow<'a, str>,
    pub title: Option<Cow<'a, str>>,
    pub format: OEmbedFormat,
}

pub type LinkList<'a> = smallvec::SmallVec<[OEmbedLink<'a>; 1]>;

pub fn parse_link_header(header: &str) -> LinkList {
    let mut res = LinkList::default();

    // multiple links can be comma-separated
    'links: for link in header.split(',') {
        let mut parts = link.split(';').map(str::trim);

        let url = match parts.next() {
            Some(url) if url.starts_with("<http") && url.ends_with('>') => &url[1..url.len() - 1],
            _ => continue,
        };

        let mut link = OEmbedLink {
            url: url.into(),
            title: None,
            format: OEmbedFormat::JSON,
        };

        //while let Some(part) = parts.next() {
        for part in parts {
            let Some((left, right)) = part.split_once('=') else {
                continue 'links;
            };

            if left == "type" && right.contains("xml") {
                link.format = OEmbedFormat::XML;
                continue;
            }

            let right = super::trim_quotes(right);

            match left {
                "title" => link.title = Some(right.into()),
                "rel" if right != "alternate" => continue 'links,
                _ => continue,
            }
        }

        res.push(link);
    }

    res.sort_by_key(|r| r.format);

    res
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OEmbedType {
    Photo,
    Video,
    Link,
    Rich,

    #[serde(other)]
    Unknown,
}

use std::borrow::Cow;

use embed::thin_str::ThinString;
use smol_str::SmolStr;

use super::StringHelpers;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OEmbed {
    pub version: OEmbedVersion1,

    #[serde(rename = "type")]
    pub kind: OEmbedType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<ThinString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_name: Option<SmolStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_url: Option<ThinString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<SmolStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_url: Option<ThinString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_age: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<ThinString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_width: Option<Integer64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_height: Option<Integer64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<ThinString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<ThinString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Integer64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Integer64>,
}

impl OEmbed {
    pub const fn is_valid(&self) -> bool {
        let has_dimensions = self.width.is_some() && self.height.is_some();

        match self.kind {
            OEmbedType::Video | OEmbedType::Rich => self.html.is_some() && has_dimensions,
            OEmbedType::Photo => self.url.is_some() && has_dimensions,
            _ => true,
        }
    }

    /// oEmbed cannot be trusted, see Matrix Synapse issue 14708
    pub fn decode_html_entities(&mut self) {
        self.title.decode_html_entities();
        self.author_name.decode_html_entities();
        self.author_url.decode_html_entities();
    }
}

/// Value that can only serialize and deserialize to `"1.0"`, `1`, or `1.0` (float)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OEmbedVersion1;

const _: () = {
    use serde::de::{self, Deserialize, Deserializer};
    use serde::ser::{Serialize, Serializer};

    impl Serialize for OEmbedVersion1 {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str("1.0")
        }
    }

    impl<'de> Deserialize<'de> for OEmbedVersion1 {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            return deserializer.deserialize_any(Visitor);

            struct Visitor;

            impl de::Visitor<'_> for Visitor {
                type Value = OEmbedVersion1;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("Literal string \"1.0\" or integer 1")
                }

                fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    if v == 1 {
                        return Ok(OEmbedVersion1);
                    }

                    Err(E::custom(format!("Invalid OEmbed Version: {v}")))
                }

                fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    if v == 1 {
                        return Ok(OEmbedVersion1);
                    }

                    Err(E::custom(format!("Invalid OEmbed Version: {v}")))
                }

                fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    if v == 1.0 {
                        return Ok(OEmbedVersion1);
                    }

                    Err(E::custom(format!("Invalid OEmbed Version: {v}")))
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    if v == "1.0" {
                        return Ok(OEmbedVersion1);
                    }

                    Err(E::custom(format!("Invalid OEmbed Version: \"{v}\"")))
                }
            }
        }
    }
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Integer64(pub i64);

const _: () = {
    use serde::de::{self, Deserialize, Deserializer};
    use serde::ser::{Serialize, Serializer};

    impl Serialize for Integer64 {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_i64(self.0)
        }
    }

    impl<'de> Deserialize<'de> for Integer64 {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            return deserializer.deserialize_any(Visitor);

            struct Visitor;

            impl de::Visitor<'_> for Visitor {
                type Value = Integer64;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("Literal Integer of numeric String")
                }

                fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(Integer64(v))
                }

                fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(Integer64(v.try_into().map_err(E::custom)?))
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    match v.parse() {
                        Ok(v) => Ok(Integer64(v)),
                        Err(e) => Err(E::custom(e)),
                    }
                }
            }
        }
    }
};
