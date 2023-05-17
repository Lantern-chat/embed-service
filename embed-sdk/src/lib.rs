#![cfg_attr(not(feature = "pg"), no_std)]

extern crate alloc;

#[macro_use]
extern crate serde;

pub use common::fixed::FixedStr;
pub use smol_str::SmolStr;
pub use timestamp::Timestamp;

#[cfg(feature = "thin-vec")]
pub use thin_vec::ThinVec as MaybeThinVec;

#[cfg(not(feature = "thin-vec"))]
pub type MaybeThinVec<T> = alloc::vec::Vec<T>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
