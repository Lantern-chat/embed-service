pub mod embed;
pub mod feed;
pub mod html;
pub mod oembed;
pub mod patterns;
pub mod quirks;
pub mod utils;

#[inline]
fn trim_quotes(s: &str) -> &str {
    s.trim_matches(|c: char| ['"', '\'', '“', '”'].contains(&c) || c.is_whitespace())
}

#[rustfmt::skip]
pub mod regexes {
    use regex::Regex;
    use std::sync::LazyLock;

    pub static ATTRIBUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?x)
            [a-zA-Z_][0-9a-zA-Z\-_]+\s*=\s*(
            ("(?:\\"|[^"])*[^\\]")| # name="value"
            ('(?:\\'|[^'])*[^\\]')| # name='value'
            ([^'"](?:\\\s|[^\s>]*)) # name=value or name=value>
        )"#).unwrap()
    });

    pub static META_TAGS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?x)
            <(?i)( # NOTE: Tags are case-insensitive
            meta\x20|                   # Regular meta tags
            title[^>]*>|                # <title> element, skipping over attributes
            link\x20|                   # link elements
            ((div|span)[^>]+itemscope)  # itemscopes
        )").unwrap()
    });
}

/// We can't embed infinite text, so this attempts to trim it below `max_len` without abrubtly
/// cutting off. It will find punctuation nearest to the limit and trim to there, or
pub fn trim_text(mut text: &str, max_len: usize) -> &str {
    text = text.trim(); // basic ws trim first

    if text.len() <= max_len {
        return text;
    }

    text = &text[..max_len];

    // try to find punctuation
    for (idx, char) in text.char_indices().rev() {
        if matches!(char, '.' | ',' | '!' | '?' | '\n') {
            return text[..idx].trim_end();
        }
    }

    text
}

use std::borrow::Cow;

pub trait StringHelpers {
    fn is_empty(&self) -> bool;

    fn trim_text(&mut self, max_len: usize);
    fn decode_html_entities(&mut self);
}

impl StringHelpers for String {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn trim_text(&mut self, max_len: usize) {
        *self = trim_text(self, max_len).to_owned();
    }

    fn decode_html_entities(&mut self) {
        if let Cow::Owned(decoded) = html_escape::decode_html_entities(self) {
            *self = decoded;
        }
    }
}

impl StringHelpers for smol_str::SmolStr {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn trim_text(&mut self, max_len: usize) {
        *self = trim_text(self, max_len).into();
    }

    fn decode_html_entities(&mut self) {
        if let Cow::Owned(decoded) = html_escape::decode_html_entities(self) {
            *self = decoded.into();
        }
    }
}

impl StringHelpers for ::embed::thin_str::ThinString {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn trim_text(&mut self, max_len: usize) {
        *self = trim_text(self, max_len).into();
    }

    fn decode_html_entities(&mut self) {
        if let Cow::Owned(decoded) = html_escape::decode_html_entities(self) {
            *self = decoded.into();
        }
    }
}

impl<T> StringHelpers for Option<T>
where
    T: StringHelpers,
{
    fn is_empty(&self) -> bool {
        match self {
            Some(inner) => inner.is_empty(),
            None => true,
        }
    }

    fn trim_text(&mut self, max_len: usize) {
        if let Some(ref mut inner) = self {
            inner.trim_text(max_len);

            if inner.is_empty() {
                *self = None;
            }
        }
    }

    fn decode_html_entities(&mut self) {
        if let Some(ref mut inner) = self {
            inner.decode_html_entities();
        }
    }
}
