// use winnow::{ascii, combinator as c, prelude::*};

// use ascii::Caseless;

// pub fn parse_adult_rating<'a>(input: &'a mut &str) -> PResult<&'a str> {
//     c::alt([
//         Caseless("adult"),
//         Caseless("mature"),
//         Caseless("RTA-5042-1996-1400-1577-RTA"),
//     ])
//     .parse_next(input)
// }

pub fn contains_adult_rating(input: &[u8]) -> bool {
    use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
    use std::sync::LazyLock;

    static ADULT_RATING: LazyLock<AhoCorasick> = LazyLock::new(|| {
        AhoCorasickBuilder::new()
            .ascii_case_insensitive(true)
            .build(["adult", "mature", "RTA-5042-1996-1400-1577-RTA"])
            .unwrap()
    });

    ADULT_RATING.is_match(input)
}
