#![cfg_attr(not(feature = "pg"), no_std)]

extern crate alloc;

#[macro_use]
extern crate serde;

pub use common::fixed::FixedStr;
pub use smol_str::SmolStr;
pub use timestamp::Timestamp;

use alloc::vec::Vec;

/// Default type returned by the embed server
///
/// You probably want to deserialise the payloads with this type alias
pub type EmbedWithExpire = (Timestamp, Embed);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
#[serde(tag = "v")]
pub enum Embed {
    #[serde(rename = "1")]
    V1(EmbedV1),
}

pub mod v1;
pub use v1::*;

impl Embed {
    pub fn url(&self) -> Option<&str> {
        match self {
            Embed::V1(embed) => embed.url.as_ref().map(|x| x as _),
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rkyv() {
        _ = rkyv::check_archived_root::<Embed>(&[]);
    }
}
