use smol_str::SmolStr;

use embed::*;
use thin_str::ThinString;

use super::html::{Header, LinkType, MetaProperty, Scope};
use super::oembed::{OEmbed, OEmbedFormat, OEmbedLink, OEmbedType};

#[derive(Debug, Default)]
pub struct ExtraFields<'a> {
    pub max_age: Option<u64>,
    pub link: Option<OEmbedLink<'a>>,
    pub manifest: Option<String>,
}

fn parse_color(color: &str) -> Option<u32> {
    match csscolorparser::parse(color) {
        Err(_) => None,
        Ok(color) => Some(u32::from_le_bytes({
            let mut bytes = color.to_rgba8();
            // rgba -> bgr0
            bytes[3] = bytes[0]; // r -> a
            bytes[0] = bytes[2]; // b -> r
            bytes[2] = bytes[3]; // a -> b
            bytes[3] = 0; // 0 -> a
            bytes
        })),
    }
}

#[allow(dead_code)] // until we add anything with VideoObject
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PossibleScopes {
    Author,
    Video,
}

impl PossibleScopes {
    pub fn id(self) -> &'static str {
        match self {
            PossibleScopes::Author => "author",
            PossibleScopes::Video => "video",
        }
    }

    pub fn ty(self) -> &'static str {
        match self {
            PossibleScopes::Author => "Person",
            PossibleScopes::Video => "VideoObject",
        }
    }
}

fn is_scope(scope: &Option<Scope>, p: PossibleScopes) -> bool {
    if let Some(scope) = scope {
        if matches!(scope.id, Some(ref id) if id == p.id())
            || matches!(scope.prop, Some(ref prop) if prop == p.id())
        {
            return true;
        }

        if let Some(ref ty) = scope.ty {
            if ty.contains(p.ty()) {
                return true;
            }
        }
    }

    false
}

// #[derive(Debug)]
// struct Image<'a> {
//     url: Cow<'a, str>,
//     width: Option<u32>,
//     height: Option<u32>,
//     mime: Option<Cow<'a, str>>,
// }

/// Build an initial embed profile from HTML meta tags
///
/// NOTE: HEADERS MUST BE SORTED BY PROPERTY NAME FOR OPTIMAL RESULTS
pub fn parse_meta_to_embed<'a>(embed: &mut EmbedV1, headers: &[Header<'a>]) -> ExtraFields<'a> {
    let mut extra = ExtraFields::default();

    #[derive(Default, Clone, Copy)]
    struct Misc<'a> {
        label: Option<&'a str>,
        data: Option<&'a str>,
    }

    let mut misc: [Misc; 4] = [Misc::default(); 4];
    let mut max_dim = 0;
    //let mut images = Vec::new();

    macro_rules! get {
        ($e:ident $(. $rest:ident)*) => {
            embed.$e $(.$rest)*.get_or_insert_with(Default::default)
        };
    }

    macro_rules! get_img {
        () => {{
            if embed.imgs.is_empty() {
                embed.imgs.push(EmbedMedia::default());
            }
            embed.imgs.first_mut().unwrap()
        }};
    }

    for header in headers {
        match header {
            Header::Meta(meta) => {
                #[rustfmt::skip]
                macro_rules! raw_content { () => { From::from(meta.content.as_ref()) }; }
                #[rustfmt::skip]
                macro_rules! content { () => { Some(From::from(meta.content.as_ref())) }; }

                let content_int = || meta.content.parse().ok();

                match &*meta.property {
                    // special property for <title></title> values
                    "" if meta.pty == MetaProperty::Title => {
                        if embed.title.is_none() {
                            embed.title = content!();
                        }
                    }

                    "description" | "og:description" | "twitter:description" => {
                        embed.description = Some(crate::util::trim_text(&meta.content).into());
                    }
                    "theme-color" | "msapplication-TileColor" => embed.color = parse_color(&meta.content),
                    "og:site_name" => embed.provider.name = content!(),
                    // TODO: This isn't quite correct, but good enough most of the time
                    "og:url" => embed.canonical = content!(),
                    "title" | "og:title" | "twitter:title" => embed.title = content!(),

                    // YouTube uses this schema at least
                    "name"
                        if meta.pty == MetaProperty::ItemProp
                            && is_scope(&meta.scope, PossibleScopes::Author) =>
                    {
                        get!(author).name = raw_content!();
                    }
                    "url"
                        if meta.pty == MetaProperty::ItemProp
                            && is_scope(&meta.scope, PossibleScopes::Author) =>
                    {
                        get!(author).url = content!();
                    }
                    "dc:creator" | "article:author" | "book:author" => get!(author).name = raw_content!(),

                    // don't let the twitter image overwrite og images
                    "twitter:image" => match embed.imgs.first_mut() {
                        Some(image) if image.url.is_empty() => image.url = raw_content!(),
                        None => get_img!().url = raw_content!(),
                        _ => {}
                    },

                    "og:image" | "og:image:url" | "og:image:secure_url" => get_img!().url = raw_content!(),
                    "og:video" | "og:video:url" | "og:video:secure_url" => get!(video).url = raw_content!(),
                    "og:audio" | "og:audio:url" | "og:audio:secure_url" => get!(audio).url = raw_content!(),

                    "og:image:width" => get_img!().width = content_int(),
                    "og:video:width" => get!(video).width = content_int(),
                    "music:duration" => get!(audio).width = content_int(),

                    "og:image:height" => get_img!().height = content_int(),
                    "og:video:height" => get!(video).height = content_int(),

                    "og:image:type" => get_img!().mime = content!(),
                    "og:video:type" => get!(video).mime = content!(),
                    "og:audio:type" => get!(audio).mime = content!(),

                    "og:image:alt" => get_img!().description = content!(),
                    "og:video:alt" => get!(video).description = content!(),
                    "og:audio:alt" => get!(audio).description = content!(),

                    "og:ttl" => match content_int() {
                        None => {}
                        Some(ttl) => extra.max_age = Some(ttl as u64),
                    },

                    "twitter:label1" | "twitter:label2" | "twitter:label3" | "twitter:label4" => {
                        let idx = meta.property.as_bytes()[meta.property.len() - 1] - b'0';
                        misc[idx as usize - 1].label = Some(&meta.content);
                    }

                    "twitter:data1" | "twitter:data2" | "twitter:data3" | "twitter:data4" => {
                        let idx = meta.property.as_bytes()[meta.property.len() - 1] - b'0';
                        misc[idx as usize - 1].data = Some(&meta.content);
                    }

                    _ if meta.property.eq_ignore_ascii_case("rating") => parse_rating(embed, &meta.content),

                    "isFamilyFriendly" => {
                        if meta.content != "true" {
                            embed.flags |= EmbedFlags::ADULT;
                        }
                    }

                    // Twitter uses these for multi-image posts
                    // FIXME: Can also include images from replies!
                    // _ if meta.pty == MetaProperty::ItemProp
                    //     && meta.property.eq_ignore_ascii_case("contenturl") =>
                    // {
                    //     // reasonable limit for embedding
                    //     if embed.fields.len() < 4 {
                    //         embed.fields.push(EmbedField {
                    //             name: "".into(),
                    //             value: "".into(),
                    //             b: false,
                    //             img: Some(Box::new(EmbedMedia {
                    //                 url: raw_content(),
                    //                 ..EmbedMedia::default()
                    //             })),
                    //         });
                    //     }
                    // }

                    //"profile:first_name" | "profile:last_name" | "profile:username" | "profile:gender" => {
                    //    embed.fields.push(EmbedField {
                    //          inline: true,
                    //    })
                    //}
                    _ => {}
                }
            }
            Header::Link(link) if link.rel == LinkType::Icon => {
                if let Some([w, h]) = link.sizes {
                    let m = w.max(h);

                    if max_dim >= m || (max_dim != 0 && m > 256) {
                        // prefer larger icons, but try not to use too large icons, though
                        continue;
                    } else {
                        max_dim = m;

                        let media = get!(provider.icon);

                        media.width = (w > 0).then_some(w as i32);
                        media.height = (h > 0).then_some(h as i32);
                    }
                } else if max_dim > 0 {
                    // if we've already found an icon with a definite size, prefer that, skip this
                    continue;
                }

                let media = get!(provider.icon);

                media.url = link.href.as_ref().into();
                media.mime = link.mime.as_ref().map(|m| From::from(m.as_ref()));
            }
            Header::Link(link) if link.rel == LinkType::Canonical => {
                embed.canonical = Some(link.href.as_ref().into());
            }
            Header::Link(link)
                if link.rel == LinkType::Manifest
                    // we don't have credentials, so we can't use the manifest
                    && link.crossorigin.as_deref() != Some("use-credentials") =>
            {
                extra.manifest = Some(link.href.as_ref().into());
            }
            Header::Link(link) if link.rel == LinkType::Alternate => {
                let ty = match link.ty {
                    Some(ref ty) if ty.contains("oembed") => ty,
                    _ => continue,
                };

                match extra.link {
                    Some(ref mut existing) => {
                        if ty.contains("json") {
                            existing.url.clone_from(&link.href);
                            existing.title.clone_from(&link.title);

                            existing.format = OEmbedFormat::JSON;
                        }
                    }
                    None => {
                        extra.link = Some(OEmbedLink {
                            url: link.href.clone(),
                            title: link.title.clone(),
                            format: if ty.contains("xml") { OEmbedFormat::XML } else { OEmbedFormat::JSON },
                        });
                    }
                }
            }
            _ => {}
        }
    }

    for m in misc {
        if let (Some(label), Some(data)) = (m.label, m.data) {
            if label.eq_ignore_ascii_case("rating") {
                parse_rating(embed, data)
            }

            // TODO: Maybe recurse to handle more arbitrary properties?
        }
    }

    determine_embed_type(embed);

    extra
}

pub(crate) fn determine_embed_type(embed: &mut EmbedV1) {
    if embed.imgs.iter().any(|img| !img.url.is_empty()) {
        embed.ty = EmbedType::Img;
    } else if embed.ty == EmbedType::Img {
        embed.ty = EmbedType::Link;
    }

    let mut check_type = |media: &Option<Box<EmbedMedia>>, ty: EmbedType| {
        if !EmbedMedia::is_empty(media) {
            embed.ty = ty;
        } else if embed.ty == ty {
            // we thought it was this ty, it was not, so revert back to link
            embed.ty = EmbedType::Link;
        }
    };

    check_type(&embed.audio, EmbedType::Audio);
    check_type(&embed.video, EmbedType::Vid);
    check_type(&embed.obj, EmbedType::Html);
}

pub fn parse_rating(embed: &mut EmbedV1, rating: &str) {
    // NOTE: In case of multiple tags, this is additive
    if crate::parser::patterns::contains_adult_rating(rating.as_bytes()) {
        embed.flags |= EmbedFlags::ADULT;
    }
}

/// Add to/overwrite embed profile with oEmbed data
pub fn parse_oembed_to_embed(embed: &mut EmbedV1, mut o: OEmbed) -> ExtraFields {
    macro_rules! get {
        ($e:ident) => {
            embed.$e.get_or_insert_with(Default::default)
        };
    }

    // oEmbed cannot be trusted, see Matrix Synapse issue 14708
    o.decode_html_entities();

    let mut extra = ExtraFields::default();

    embed.ty = match o.kind {
        OEmbedType::Photo => EmbedType::Img,
        OEmbedType::Video => EmbedType::Vid,
        OEmbedType::Rich => EmbedType::Html,
        OEmbedType::Link => EmbedType::Link,
        OEmbedType::Unknown => embed.ty,
    };

    if o.author_name.is_some() || o.author_url.is_some() {
        let author = get!(author);

        author.url.overwrite_with(o.author_url);
        if let Some(author_name) = o.author_name {
            author.name = author_name;
        }
    }

    // QUIRK: Sometimes oEmbed returns a bad title
    // that's just a prefix of the meta tags title
    if let Some(title) = o.title {
        match embed.title {
            Some(ref t) if t.starts_with(title.as_str()) => {}
            _ => embed.title = Some(title),
        }
    }

    embed.provider.name.overwrite_with(o.provider_name);
    embed.provider.url.overwrite_with(o.provider_url);

    if embed.ty == EmbedType::Link {
        determine_embed_type(embed);
    }

    let media = match o.kind {
        OEmbedType::Photo => {
            embed.imgs.push(EmbedMedia::default());
            embed.imgs.first_mut().unwrap()
        }
        OEmbedType::Video => &mut **get!(video),
        _ => &mut **get!(obj),
    };

    let mut mime = media.mime.take();
    let mut overwrite = false;

    if let Some(html) = o.html {
        match mime {
            Some(ref mime) if mime == "text/html" => {}
            _ => {
                if let Some(src) = parse_embed_html_src(&html) {
                    media.url = src;
                    mime = Some(parse_embed_html_type(&html).unwrap_or(SmolStr::new_inline("text/html")));
                    overwrite = true;
                }
            }
        }
    } else if let Some(url) = o.url {
        media.url = url;
        mime = None; // unknown
        overwrite = true;
    }

    media.mime = mime;

    if overwrite {
        media.width = o.width.map(|x| x.0 as _);
        media.height = o.height.map(|x| x.0 as _);
    }

    if let Some(thumbnail_url) = o.thumbnail_url {
        let mut thumb = Box::<EmbedMedia>::default();

        thumb.url = thumbnail_url;
        thumb.width = o.thumbnail_width.map(|x| x.0 as _);
        thumb.height = o.thumbnail_height.map(|x| x.0 as _);

        thumb.mime = None; // unknown from here

        embed.thumb = Some(thumb);
    }

    extra.max_age.overwrite_with(o.cache_age);

    extra
}

pub trait OptionExt<T> {
    fn overwrite_with(&mut self, value: Option<T>);
}

impl<T> OptionExt<T> for Option<T> {
    #[inline]
    fn overwrite_with(&mut self, value: Option<T>) {
        if value.is_some() {
            *self = value;
        }
    }
}

fn parse_embed_html_src(html: &str) -> Option<ThinString> {
    let mut start = memchr::memmem::find(html.as_bytes(), b"src=\"http")?;

    // strip src=" prefix
    start += "src=\"".len();

    let end = start + memchr::memchr(b'"', &html.as_bytes()[start..])?;

    let src = &html[start..end];

    memchr::memmem::find(src.as_bytes(), b"://")?;

    Some(From::from(src))
}

fn parse_embed_html_type(html: &str) -> Option<SmolStr> {
    let mut start = memchr::memmem::find(html.as_bytes(), b"type=\"")?;

    start += "type=\"".len(); // strip prefix

    let end = start + memchr::memchr(b'"', &html.as_bytes()[start..])?;

    let ty = &html[start..end];

    // mime type e.g.: image/png
    memchr::memchr(b'/', ty.as_bytes())?;

    Some(From::from(ty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_embed_html() {
        let fixture = "<object width=\"425\" height=\"344\">
        <param name=\"movie\" value=\"https://www.youtube.com/v/M3r2XDceM6A&fs=1\"></param>
        <param name=\"allowFullScreen\" value=\"true\"></param>
        <param name=\"allowscriptaccess\" value=\"always\"></param>
        <embed src=\"https://www.youtube.com/v/M3r2XDceM6A&fs=1\"
            type=\"application/x-shockwave-flash\" width=\"425\" height=\"344\"
            allowscriptaccess=\"always\" allowfullscreen=\"true\"></embed>
        </object>";

        let src = parse_embed_html_src(fixture);
        let ty = parse_embed_html_type(fixture);

        assert_eq!(
            src.as_ref().map::<&str, _>(|s| s),
            Some("https://www.youtube.com/v/M3r2XDceM6A&fs=1")
        );

        assert_eq!(
            ty.as_ref().map::<&str, _>(|s| s),
            Some("application/x-shockwave-flash")
        );
    }
}
