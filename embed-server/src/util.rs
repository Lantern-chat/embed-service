use std::{
    borrow::Cow,
    fmt::{self, Write},
};

pub fn format_list<I, T>(mut out: impl Write, list: impl IntoIterator<IntoIter = I>) -> Result<(), fmt::Error>
where
    I: Iterator<Item = T>,
    T: fmt::Display,
{
    let list = list.into_iter();
    let (len, _) = list.size_hint();

    for (idx, item) in list.enumerate() {
        let delim = match idx {
            _ if (idx + 1) == len => "",
            _ if len == 2 && idx == 0 => " and ",
            _ if (idx + 2) == len => ", and ",
            _ => ", ",
        };
        write!(out, "{item}{delim}")?;
    }

    Ok(())
}

/// Removes redundant newlines from the text,
/// collapsing multiple newlines into a maximum of two.
///
/// This also removes carriage returns and trims any leading/trailing whitespace,
/// trying not to allocate as best as possible.
pub fn trim_text(text: &str) -> Cow<str> {
    let mut trimmed = Cow::Borrowed(text.trim());

    if trimmed.is_empty() {
        return trimmed;
    }

    let mut new_text = String::new();

    let mut chars = trimmed.char_indices();

    let mut last_idx = 0;

    // collapse multiple newlines into one or two
    while let Some((start_idx, c)) = chars.next() {
        let mut cnt = match c {
            '\r' => 0,
            '\n' => 1,
            _ => continue,
        };

        let mut end_idx = start_idx;

        // scan ahead to the end of the newline sequence
        for (idx, c) in chars.by_ref() {
            match c {
                '\r' => end_idx = idx,
                '\n' => {
                    cnt += 1;
                    end_idx = idx;
                }
                _ => break,
            }
        }

        // up to two plain newlines are allowed as-is
        if cnt <= 2 && (end_idx - start_idx + 1) == cnt {
            continue;
        }

        new_text.push_str(&trimmed[last_idx..start_idx]);
        last_idx = end_idx + 1;

        new_text.push_str(match cnt {
            1 => "\n",
            _ => "\n\n",
        })
    }

    if last_idx != 0 {
        new_text.push_str(trimmed[last_idx..].trim_end());

        trimmed = new_text.into();
    }

    trimmed
}

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, Anchored, Input, StartKind};

pub struct TagChecker {
    pub tags: AhoCorasick,
}

impl TagChecker {
    pub fn new<P>(tags: impl IntoIterator<Item = P>) -> Self
    where
        P: AsRef<[u8]>,
    {
        Self {
            tags: AhoCorasickBuilder::new()
                .ascii_case_insensitive(true)
                .start_kind(StartKind::Anchored)
                .build(tags)
                .unwrap(),
        }
    }

    pub fn contains<H>(&self, tag: &H) -> bool
    where
        H: ?Sized + AsRef<[u8]>,
    {
        self.tags
            .try_find(Input::new(tag).anchored(Anchored::Yes).earliest(true))
            .expect("AhoCorasick::try_find is not expected to fail")
            .is_some()
    }
}
