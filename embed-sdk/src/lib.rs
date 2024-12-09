#![no_std]

pub use client_sdk::models::embed::*;
pub use client_sdk::models::util::{fixed_str, thin_str};

pub extern crate iso8601_timestamp as timestamp;
pub extern crate smol_str;
