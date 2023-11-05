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

pub fn trim_text(text: &str) -> Cow<str> {
    let mut trimmed = Cow::Borrowed(text.trim());

    if !trimmed.is_empty() {
        let mut new_text = String::new();
        let mut idx = 0;

        for (start, end) in crate::parser::regexes::NEWLINES.find_iter(trimmed.as_bytes()) {
            new_text.push_str(&trimmed[idx..start]);
            new_text.push_str("\n\n");
            idx = end;
        }

        if idx != 0 {
            new_text.push_str(&trimmed[idx..]);

            // trim any ending whitespace
            new_text.truncate(new_text.trim_end().len());

            trimmed = new_text.into();
        }
    }

    trimmed
}
