use super::prelude::*;

use generic::RawGenericExtraction;

pub struct BlueskyExtractorFactory;

#[derive(Default, Debug)]
pub struct BlueskyExtractor {}

impl ExtractorFactory for BlueskyExtractorFactory {
    fn create(&self, config: &Config) -> Result<Option<Box<dyn Extractor>>, ConfigError> {
        // only once we have something to configure
        // let Some(extractor) = config.parsed.extractors.get("bluesky") else {
        //     return Ok(None);
        // };

        let mut bsky = Box::<BlueskyExtractor>::default();

        Ok(Some(bsky))
    }
}

pub mod models;
use models::*;

#[async_trait::async_trait]
impl Extractor for BlueskyExtractor {
    #[allow(clippy::match_like_matches_macro)]
    fn matches(&self, url: &Url) -> bool {
        matches!(url.domain(), Some("bsky.app"))
    }

    async fn setup(&self, state: Arc<ServiceState>) -> Result<(), Error> {
        Ok(())
    }

    async fn extract(
        &self,
        state: Arc<ServiceState>,
        url: Url,
        params: Params,
    ) -> Result<EmbedWithExpire, Error> {
        // first segment is always empty, because of the leading slash
        let mut segments = url.path().split('/').skip(1);

        let raw = match segments.next() {
            Some("profile") => 'extract: {
                let Some(handle) = segments.next() else {
                    return Err(Error::Failure(StatusCode::NOT_FOUND));
                };

                let mut post_id = None;

                match segments.next() {
                    None => {}
                    Some("post") => post_id = segments.next(),
                    _ => break 'extract generic::extract_raw(state, url, params).boxed().await?,
                }

                let get_profile =
                    format!("https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile?actor={handle}");

                let resp = state.client.get(get_profile).send().await?;

                if !resp.status().is_success() {
                    return Err(Error::Failure(resp.status()));
                }

                let profile: BskyProfile = resp.json().await?;

                let mut embed = EmbedV1::default();

                embed.title = Some(format_thin_string!("@{}", profile.handle));

                embed.author = Some({
                    let mut author = EmbedAuthor::default();

                    author.url = Some(format_thin_string!("https://bsky.app/profile/{}", profile.handle));

                    let mut name = profile.display_name;
                    if name.is_empty() {
                        name = profile.handle;
                    }

                    author.name = name;
                    author.icon = Some(Box::<EmbedMedia>::default().with_url(profile.avatar));

                    author
                });

                let mut footer = EmbedFooter::default();

                embed.flags |= BskyLabel::aggregate_flags(&profile.labels);

                // if this was just a profile request, we're done, finish up and return
                let Some(post_id) = post_id else {
                    embed.description = Some(profile.description);
                    embed.url = embed.author.as_ref().unwrap().url.clone();

                    break 'extract RawGenericExtraction {
                        state,
                        embed,
                        // 4-hour expire
                        max_age: Some(60 * 60 * 4),
                    };
                };

                let get_post = format!("https://public.api.bsky.app/xrpc/app.bsky.feed.getPosts?uris=at://{}/app.bsky.feed.post/{post_id}", profile.did);

                let resp = state.client.get(get_post).send().await?;

                if !resp.status().is_success() {
                    return Err(Error::Failure(resp.status()));
                }

                let mut posts: BskyPosts = resp.json().await?;

                let Some(post) = posts.posts.pop() else {
                    return Err(Error::Failure(StatusCode::NOT_FOUND));
                };

                embed.flags |= BskyLabel::aggregate_flags(&post.labels);

                // handle unknown record/embed types in one place
                match (&post.record, &post.embed) {
                    (BskyRecord::Unknown, _) | (_, BskyEmbed::Unknown) => {
                        break 'extract generic::extract_raw(state, url, params).boxed().await?;
                    }
                    _ => {}
                }

                let mut ts = None;

                match post.record {
                    BskyRecord::Unknown => unreachable!(),

                    // nested embeds won't ever appear here
                    BskyRecord::Post { created_at, text, .. } => {
                        ts = Some(created_at);
                        embed.description = Some(text);
                    }

                    BskyRecord::Record { .. } => {}
                }

                embed.footer = Some({
                    let mut footer = EmbedFooter::default();

                    write_footer(
                        &mut footer.text,
                        ts,
                        post.like_count,
                        post.reply_count,
                        post.repost_count,
                        post.quote_count,
                    )
                    .unwrap();

                    footer
                });

                let bsky_embed = match post.embed {
                    BskyEmbed::RecordWithMedia { media, record } => {
                        if let BskyRecord::Record { value, author, .. } = record.record {
                            // can't nest this in the pattern match due to boxing
                            if let BskyRecord::Post { text, .. } = *value {
                                append_description(
                                    embed.description.get_or_insert_default(),
                                    &author.display_name,
                                    &author.handle,
                                    &text,
                                )
                                .unwrap();
                            }
                        }

                        *media
                    }
                    BskyEmbed::Record { record } => match *record {
                        BskyRecord::Record {
                            value,
                            author,
                            mut embeds,
                        } => {
                            // can't nest this in the pattern match due to boxing
                            if let BskyRecord::Post { text, .. } = *value {
                                append_description(
                                    embed.description.get_or_insert_default(),
                                    &author.display_name,
                                    &author.handle,
                                    &text,
                                )
                                .unwrap();

                                embeds.pop().map(|embed| embed.embed()).unwrap_or(BskyEmbed::Unknown)
                            } else {
                                BskyEmbed::Unknown
                            }
                        }
                        _ => BskyEmbed::Unknown,
                    },
                    bsky_embed => bsky_embed,
                };

                match bsky_embed {
                    BskyEmbed::Unknown | BskyEmbed::RecordWithMedia { .. } => {}

                    BskyEmbed::External { external } => {
                        embed.url = Some(external.uri);

                        if !external.title.is_empty() {
                            let desc = embed.description.get_or_insert_default();

                            write!(desc, "\n\n> **{}**", external.title).unwrap();
                        }

                        if !external.description.is_empty() {
                            let desc = embed.description.get_or_insert_default();

                            for line in external.description.lines() {
                                write!(desc, "\n\n> {}", line.trim()).unwrap();
                            }
                        }
                    }
                    BskyEmbed::Images { images } => {
                        let mut media = Box::<EmbedMedia>::default();

                        for image in images {
                            let mut img = BasicEmbedMedia::default();

                            img.url = match image.thumb.is_empty() {
                                true => image.fullsize,
                                false => image.thumb,
                            };
                            img.description = Some(image.alt);
                            img.width = Some(image.aspect_ratio.width as i32);
                            img.height = Some(image.aspect_ratio.height as i32);
                            img.mime = Some("image/jpeg".into());

                            media.alts.push(img);
                        }

                        embed.img = Some(media);
                    }
                    BskyEmbed::Video { video, .. } => {
                        if !video.thumbnail.is_empty() {
                            embed.thumb = Some({
                                Box::<EmbedMedia>::default()
                                    .with_dims(
                                        video.aspect_ratio.width as i32,
                                        video.aspect_ratio.height as i32,
                                    )
                                    .with_url(video.thumbnail)
                                    .with_mime("image/jpeg")
                            });
                        }

                        embed.video = Some({
                            Box::<EmbedMedia>::default()
                                .with_dims(video.aspect_ratio.width as i32, video.aspect_ratio.height as i32)
                                .with_url(video.playlist)
                                .with_mime("application/mpegurl")
                        });
                    }

                    BskyEmbed::Record { .. } => {}
                }

                RawGenericExtraction {
                    state,
                    embed,
                    // 4-hour expire
                    max_age: Some(60 * 60 * 4),
                }
            }
            _ => generic::extract_raw(state, url, params).boxed().await?,
        };

        let RawGenericExtraction {
            state,
            mut embed,
            max_age,
        } = raw;

        embed.color = Some(0x208bfe);
        embed.provider.name = Some(SmolStr::new_inline("Bluesky Social"));
        embed.provider.url = Some(ThinString::from("https://bsky.app/"));
        embed.provider.icon = Some(
            Box::<EmbedMedia>::default()
                .with_url("https://bsky.app/static/apple-touch-icon.png")
                .with_dims(180, 180)
                .with_description("Bluesky Social"),
        );

        Ok(generic::finalize_embed(state, embed, max_age))
    }
}
