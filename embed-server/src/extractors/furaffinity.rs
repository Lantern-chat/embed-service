use super::prelude::*;

use ego_tree::iter::Edge;
use scraper::{CaseSensitivity::AsciiCaseInsensitive, ElementRef, Node, Selector};

macro_rules! selector {
    ($e: expr) => {{
        static SELECTOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse($e).unwrap());
        &*SELECTOR
    }};
}

pub struct FurAffinityExtractorFactory;

impl ExtractorFactory for FurAffinityExtractorFactory {
    fn create(&self, config: &Config) -> Result<Option<Box<dyn Extractor>>, ConfigError> {
        let Some(extractor) = config.parsed.extractors.get("furaffinity") else {
            return Ok(None);
        };

        let Some(a) = extractor.get("a") else {
            return Err(ConfigError::MissingExtractorField("furaffinity.a"));
        };

        let Some(b) = extractor.get("b") else {
            return Err(ConfigError::MissingExtractorField("furaffinity.b"));
        };

        let Some(ua) = config.parsed.user_agents.get("%browser") else {
            return Err(ConfigError::InvalidUserAgent("%browser not found".to_owned()));
        };

        let Ok(cookie) = HeaderValue::try_from(format!("b={b}; a={a}")) else {
            return Err(ConfigError::InvalidExtractorField("furaffinity.(a|b)"));
        };

        Ok(Some(Box::new(FurAffinityExtractor {
            cookie,
            user_agent: ua.0.clone(),
        })))
    }
}

#[derive(Debug)]
pub struct FurAffinityExtractor {
    pub cookie: HeaderValue,
    pub user_agent: HeaderValue,
}

#[async_trait::async_trait]
impl Extractor for FurAffinityExtractor {
    fn matches(&self, url: &Url) -> bool {
        matches!(url.domain(), Some("furaffinity.net" | "www.furaffinity.net"))
            && url.path().starts_with("/view/")
    }

    #[instrument(skip_all)]
    async fn extract(
        &self,
        state: Arc<ServiceState>,
        url: Url,
        params: Params,
    ) -> Result<EmbedWithExpire, Error> {
        let resp = state
            .client
            .get(url.clone())
            .header(HeaderName::from_static("cookie"), &self.cookie)
            .header(HeaderName::from_static("user-agent"), &self.user_agent)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::Failure(resp.status()));
        }

        let html = resp.text().await?;

        let ParsedResult {
            mut embed,
            needs_media_resolved,
        } = parse_html(&html, &url)?;

        if needs_media_resolved {
            generic::resolve_media::resolve_images(&state, &None, &mut embed).await?;
        }

        static FA_PROVIDER: LazyLock<EmbedProvider> = LazyLock::new(|| {
            let mut provider = EmbedProvider::default();

            provider.name = Some(SmolStr::new_inline("FurAffinity"));
            provider.url = Some(ThinString::from("https://www.furaffinity.net"));
            provider.icon = Some(
                Box::<EmbedMedia>::default()
                    .with_url("https://www.furaffinity.net/themes/beta/img/favicon.ico"),
            );

            provider
        });

        embed.provider = FA_PROVIDER.clone();

        Ok(generic::finalize_embed(state, embed, Some(60 * 60 * 4)))
    }
}

fn trim_nl(t: &str) -> &str {
    t.trim_matches(|c: char| matches!(c, '\r' | '\n'))
}

fn fix_relative_scheme(url: &str) -> Cow<str> {
    match url.strip_prefix("//") {
        Some(url) => Cow::Owned(format!("https://{url}")),
        None => Cow::Borrowed(url),
    }
}

fn accumulate_text(el: ElementRef) -> String {
    el.text().fold(String::new(), |mut a, chunk| {
        a.push_str(chunk);
        a
    })
}

struct ParsedResult {
    embed: EmbedV1,
    needs_media_resolved: bool,
}

fn parse_html(html: &str, url: &Url) -> Result<ParsedResult, Error> {
    let doc = scraper::Html::parse_document(html);

    let mut embed = EmbedV1::default();

    #[derive(Debug, PartialEq, Eq)]
    enum Kind {
        Image,
        Video,
        Audio,
        Unsupported,
    }

    let mut needs_media_resolved = false;

    // find submission and parse media nodes
    if let Some(node) = doc.select(selector!("div.submission-area")).next() {
        let mut src = None;
        let mut alt = None;
        let mut kind = Kind::Unsupported;

        for e in node.traverse() {
            let Edge::Open(node) = e else { continue };

            if let Node::Element(el) = node.value() {
                kind = match el.name() {
                    "img" => Kind::Image,
                    "audio" => Kind::Audio,
                    "vid" => Kind::Video,
                    "object" => break,
                    _ => continue,
                };

                src = el.attr("src");
                alt = el.attr("alt");

                break;
            }
        }

        match src {
            Some(src) if kind != Kind::Unsupported => {
                let use_thumbnail = node.value().has_class("submission-writing", AsciiCaseInsensitive);

                let mut media = Box::<EmbedMedia>::default().with_url(fix_relative_scheme(src)).guess_mime();

                media.description = alt.map(ThinString::from);

                if matches!(kind, Kind::Image | Kind::Video) {
                    'highlights: for info_node in doc.select(selector!("div.info > div > strong.highlight")) {
                        if !matches!(info_node.text().next(), Some(txt) if txt.eq_ignore_ascii_case("size")) {
                            continue;
                        }

                        // for each sibling node, try to find the size text and parse it
                        for sibling in info_node.next_siblings() {
                            // only care about span elements
                            let el = match ElementRef::wrap(sibling) {
                                Some(el) if el.value().name() == "span" => el,
                                _ => continue,
                            };

                            // the span node should contain the size in the format "WIDTH x HEIGHTpx"
                            if let Some((width, height)) = el
                                .text()
                                .next()
                                .and_then(|size_text| size_text.trim_end_matches("px").split_once('x'))
                            {
                                media.width = width.trim().parse().ok();
                                media.height = height.trim().parse().ok();
                                break 'highlights;
                            }
                        }
                    }
                }

                if media.mime.is_none() || media.width.is_none() || media.height.is_none() {
                    needs_media_resolved = true;
                }

                match kind {
                    Kind::Image if use_thumbnail => embed.thumb = Some(media),
                    Kind::Image => embed.imgs.push(*media),
                    Kind::Video => embed.video = Some(media),
                    Kind::Audio => embed.audio = Some(media),
                    Kind::Unsupported => unreachable!(),
                }
            }
            _ => {}
        }
    }

    // aggregate description text
    if let Some(node) = doc.select(selector!("div.submission-description")).next() {
        let mut description = String::new();

        for e in node.traverse() {
            let Edge::Open(node) = e else {
                continue;
            };
            description += match node.value() {
                Node::Text(t) => trim_nl(t).trim_start(),
                Node::Element(el) => match el.name() {
                    "br" if !description.ends_with("\n\n") => "\n",
                    "img" => match el.attr("alt") {
                        Some(alt_text) => {
                            // in some cases, there can be duplicate text of the alt name right next to the img element
                            if let Some(text) = node.next_sibling().and_then(|s| s.value().as_text()) {
                                if alt_text == text.trim() {
                                    continue;
                                }
                            }

                            alt_text
                        }
                        None => continue,
                    },
                    _ => continue,
                },
                _ => continue,
            };
        }

        description.truncate(description.trim_end().len());
        embed.description = Some(description.into());
    }

    let mut author = EmbedAuthor::default();

    if let Some(node) = doc.select(selector!("div.submission-title")).next() {
        embed.title = Some(accumulate_text(node).into());

        for sibling in node.next_siblings() {
            if let Some(a) = ElementRef::wrap(sibling) {
                // <a href="/user/AUTHOR">
                match a.value().attr("href") {
                    Some(href) if href.starts_with("/user/") => {
                        author.url = Some(format!("https://www.furaffinity.net{href}").into());
                        author.name = accumulate_text(a).into();
                        break;
                    }
                    _ => continue,
                }
            }
        }
    }

    if let Some(node) = doc.select(selector!("img.submission-user-icon")).next() {
        if let Some(src) = node.value().attr("src") {
            author.icon = Some(Box::<EmbedMedia>::default().with_url(fix_relative_scheme(src)).guess_mime());
        }
    }

    embed.author = Some(author);

    if let Some(node) = doc.select(selector!("span.rating-box")).next() {
        if !node.value().has_class("general", AsciiCaseInsensitive) {
            embed.flags |= EmbedFlags::ADULT;
        }
    }

    // attempt to find additional content tags, since FA isn't great at enforcing
    // the rating system.
    for tag in doc.select(selector!("span.tags > a")) {
        // tag elements only contain one text node
        if let Some(tag) = tag.text().next() {
            // FA has a very lax rating system, so we have to be very specific, unfortunately.
            // These are on a best-effort basis, as I don't have the ability or desire to
            // manually look through every tag. Honestly annoying this stuff slips through.
            #[rustfmt::skip]
            static ADULT_TAGS: LazyLock<TagChecker> = LazyLock::new(|| {
                TagChecker::new([
                    "nsfw", "sex", "horny", "r18", "fetish", "hentai", "yiff",
                    "rape", "necrophilia", "vore", "hyper", "clit",
                    "erection", "penis", "cum", "pussy", "dick",
                    "porn", "ssbbw", "immobility", "ussbbw",
                ])
            });

            // don't check for adult tags if the embed is already marked as adult
            if !embed.flags.contains(EmbedFlags::ADULT) && ADULT_TAGS.contains(tag) {
                embed.flags |= EmbedFlags::ADULT;
            }

            // Again, not happy I have to list these.
            static GRAPHIC_TAGS: LazyLock<TagChecker> =
                LazyLock::new(|| TagChecker::new(["gore", "snuff", "necrophilia"]));

            if !embed.flags.contains(EmbedFlags::GRAPHIC) && GRAPHIC_TAGS.contains(tag) {
                embed.flags |= EmbedFlags::GRAPHIC;
            }

            if embed.flags.contains(EmbedFlags::ADULT | EmbedFlags::GRAPHIC) {
                break;
            }
        }
    }

    embed.url = Some({
        let mut origin = url.origin().ascii_serialization();
        origin += url.path();
        origin.into()
    });

    embed.color = Some(0xadd8f5);

    Ok(ParsedResult {
        embed,
        needs_media_resolved,
    })
}
