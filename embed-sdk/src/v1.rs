use super::*;

use core::ops::{Deref, DerefMut};

use alloc::boxed::Box;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
#[serde(rename_all = "lowercase")]
pub enum EmbedType {
    #[serde(alias = "image")]
    Img,
    Audio,
    #[serde(alias = "video")]
    Vid,
    Html,
    Link,
    Article,
}

bitflags::bitflags! {
    pub struct EmbedFlags: i16 {
        /// This embed contains spoilered content and should be displayed as such
        const SPOILER   = 1 << 0;

        /// This embed may contain content marked as "adult"
        ///
        /// NOTE: This is not always accurate, and is provided on a best-effort basis
        const ADULT     = 1 << 1;
    }
}

common::impl_serde_for_bitflags!(EmbedFlags);
common::impl_schema_for_bitflags!(EmbedFlags);
common::impl_sql_for_bitflags!(EmbedFlags);

fn is_none_or_empty(value: &Option<SmolStr>) -> bool {
    match value {
        Some(ref value) => value.is_empty(),
        None => true,
    }
}

/// An embed is metadata taken from a given URL by loading said URL, parsing any meta tags, and fetching
/// extra information from oEmbed sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct EmbedV1 {
    /// Timestamp when the embed was retreived
    #[cfg_attr(feature = "typed-builder", builder(default = Timestamp::now_utc()))]
    pub ts: Timestamp,

    /// Embed type
    #[serde(alias = "type")]
    pub ty: EmbedType,

    #[serde(
        rename = "f",
        alias = "flags",
        default = "EmbedFlags::empty",
        skip_serializing_if = "EmbedFlags::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default = EmbedFlags::empty()))]
    pub flags: EmbedFlags,

    /// URL fetched
    #[serde(
        rename = "u",
        alias = "url",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub url: Option<SmolStr>,

    /// Canonical URL
    #[serde(
        rename = "c",
        alias = "canonical",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub canonical: Option<SmolStr>,

    #[serde(
        rename = "t",
        alias = "title",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub title: Option<SmolStr>,

    /// Description, usually from the Open-Graph API
    #[serde(
        rename = "d",
        alias = "description",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub description: Option<SmolStr>,

    /// Accent Color
    #[serde(
        rename = "ac",
        default,
        skip_serializing_if = "Option::is_none",
        alias = "color"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub color: Option<u32>,

    #[serde(rename = "au", default, skip_serializing_if = "EmbedAuthor::is_none")]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub author: Option<EmbedAuthor>,

    /// oEmbed Provider
    #[serde(
        rename = "p",
        alias = "provider",
        default,
        skip_serializing_if = "EmbedProvider::is_none"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub provider: EmbedProvider,

    /// HTML and similar objects
    ///
    /// See: <https://www.html5rocks.com/en/tutorials/security/sandboxed-iframes/>
    #[serde(default, skip_serializing_if = "EmbedMedia::is_empty")]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub obj: Option<BoxedEmbedMedia>,
    #[serde(default, skip_serializing_if = "EmbedMedia::is_empty", alias = "image")]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub img: Option<BoxedEmbedMedia>,
    #[serde(default, skip_serializing_if = "EmbedMedia::is_empty")]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub audio: Option<BoxedEmbedMedia>,
    #[serde(
        rename = "vid",
        alias = "video",
        default,
        skip_serializing_if = "EmbedMedia::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub video: Option<BoxedEmbedMedia>,
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[serde(default, skip_serializing_if = "EmbedMedia::is_empty")]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub thumb: Option<BoxedEmbedMedia>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub fields: Vec<EmbedField>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub footer: Option<EmbedFooter>,
}

impl EmbedV1 {
    pub fn has_fullsize_media(&self) -> bool {
        !EmbedMedia::is_empty(&self.obj)
            || !EmbedMedia::is_empty(&self.img)
            || !EmbedMedia::is_empty(&self.audio)
            || !EmbedMedia::is_empty(&self.video)
    }

    // NOTE: Provider, canonical, and title can be skipped here, as by themselves it's a very boring embed
    pub fn is_plain_link(&self) -> bool {
        if self.ty != EmbedType::Link
            || self.url.is_none()
            || !is_none_or_empty(&self.description)
            || self.color.is_some()
            || !EmbedAuthor::is_none(&self.author)
            || self.has_fullsize_media()
            || !EmbedMedia::is_empty(&self.thumb)
            || !self.fields.is_empty()
            || self.footer.is_some()
        {
            return false;
        }

        true
    }

    pub fn visit_text_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut SmolStr),
    {
        fn visit_text_opt_mut<F>(text: &mut Option<SmolStr>, mut f: F)
        where
            F: FnMut(&mut SmolStr),
        {
            if let Some(ref mut value) = *text {
                f(value);
            }
        }

        visit_text_opt_mut(&mut self.title, &mut f);
        visit_text_opt_mut(&mut self.description, &mut f);
        visit_text_opt_mut(&mut self.provider.name, &mut f);

        if let Some(ref mut author) = self.author {
            f(&mut author.name);
        }

        self.visit_media(|media| visit_text_opt_mut(&mut media.description, &mut f));

        for field in &mut self.fields {
            f(&mut field.name);
            f(&mut field.value);
        }

        if let Some(ref mut footer) = self.footer {
            f(&mut footer.text);
        }
    }

    pub fn visit_full_media<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut EmbedMedia),
    {
        fn visit_opt<F>(value: &mut Option<BoxedEmbedMedia>, mut f: F)
        where
            F: FnMut(&mut EmbedMedia),
        {
            if let Some(media) = value {
                f(media);
            }
        }

        visit_opt(&mut self.obj, &mut f);
        visit_opt(&mut self.img, &mut f);
        visit_opt(&mut self.audio, &mut f);
        visit_opt(&mut self.video, &mut f);
        visit_opt(&mut self.thumb, &mut f);
        visit_opt(&mut self.provider.icon, &mut f);

        if let Some(ref mut footer) = self.footer {
            visit_opt(&mut footer.icon, &mut f);
        }

        if let Some(ref mut author) = self.author {
            visit_opt(&mut author.icon, &mut f);
        }

        for field in &mut self.fields {
            visit_opt(&mut field.img, &mut f);
        }
    }
}

impl VisitMedia for EmbedV1 {
    /// Visit each [`EmbedMedia`] to mutate them (such as to generate the proxy signature)
    fn visit_media<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut BasicEmbedMedia),
    {
        self.visit_full_media(|media| {
            media.visit_media(&mut f);
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct EmbedFooter {
    #[serde(rename = "t", alias = "text")]
    #[cfg_attr(feature = "typed-builder", builder(setter(into)))]
    pub text: SmolStr,

    #[serde(
        rename = "i",
        alias = "icon",
        default,
        skip_serializing_if = "EmbedMedia::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub icon: Option<BoxedEmbedMedia>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[repr(transparent)]
pub struct BoxedEmbedMedia(Box<EmbedMedia>);

/// Rather than implement `Archive` directly on `BoxedEmbedMedia`, as all uses are as `Option<BoxedEmbedMedia>`,
/// use `ArchiveWith` to achieve niche optimizations
#[cfg(feature = "rkyv")]
pub struct NicheEmbedMedia;

#[cfg(feature = "rkyv")]
const _: () = {
    use rkyv::{
        niche::option_box::{ArchivedOptionBox, OptionBoxResolver},
        ser::Serializer,
        with::{ArchiveWith, DeserializeWith, SerializeWith},
        ArchiveUnsized, Archived, Deserialize, DeserializeUnsized, Fallible, MetadataResolver,
        SerializeUnsized,
    };

    impl ArchiveWith<Option<BoxedEmbedMedia>> for NicheEmbedMedia {
        type Archived = ArchivedOptionBox<Archived<EmbedMedia>>;
        type Resolver = OptionBoxResolver<MetadataResolver<EmbedMedia>>;

        unsafe fn resolve_with(
            field: &Option<BoxedEmbedMedia>,
            pos: usize,
            resolver: Self::Resolver,
            out: *mut Self::Archived,
        ) {
            ArchivedOptionBox::resolve_from_option(field.as_deref(), pos, resolver, out)
        }
    }

    impl<S> SerializeWith<Option<BoxedEmbedMedia>, S> for NicheEmbedMedia
    where
        EmbedMedia: SerializeUnsized<S>,
        S: Serializer + ?Sized,
    {
        fn serialize_with(
            field: &Option<BoxedEmbedMedia>,
            serializer: &mut S,
        ) -> Result<Self::Resolver, S::Error> {
            ArchivedOptionBox::serialize_from_option(field.as_deref(), serializer)
        }
    }

    impl<D> DeserializeWith<ArchivedOptionBox<Archived<EmbedMedia>>, Option<BoxedEmbedMedia>, D>
        for NicheEmbedMedia
    where
        EmbedMedia: ArchiveUnsized,
        Archived<EmbedMedia>: DeserializeUnsized<EmbedMedia, D>,
        D: Fallible + ?Sized,
    {
        fn deserialize_with(
            field: &ArchivedOptionBox<Archived<EmbedMedia>>,
            deserializer: &mut D,
        ) -> Result<Option<BoxedEmbedMedia>, D::Error> {
            if let Some(value) = field.as_ref() {
                Ok(Some(BoxedEmbedMedia(value.deserialize(deserializer)?)))
            } else {
                Ok(None)
            }
        }
    }
};

impl BoxedEmbedMedia {
    #[inline(always)]
    pub fn read(self) -> EmbedMedia {
        *self.0
    }

    #[inline]
    pub fn with_url(mut self, url: impl Into<SmolStr>) -> Self {
        self.url = url.into();
        self
    }

    #[inline]
    pub fn with_dims(mut self, width: i32, height: i32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    #[inline]
    pub fn with_mime(mut self, mime: impl Into<SmolStr>) -> Self {
        self.mime = Some(mime.into());
        self
    }

    #[inline]
    pub fn with_description(mut self, description: impl Into<SmolStr>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl Deref for BoxedEmbedMedia {
    type Target = EmbedMedia;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BoxedEmbedMedia {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<EmbedMedia> for Option<BoxedEmbedMedia> {
    fn from(value: EmbedMedia) -> Self {
        Some(value.into())
    }
}

impl From<Box<EmbedMedia>> for Option<BoxedEmbedMedia> {
    fn from(value: Box<EmbedMedia>) -> Self {
        Some(value.into())
    }
}

impl From<EmbedMedia> for BoxedEmbedMedia {
    fn from(value: EmbedMedia) -> Self {
        BoxedEmbedMedia(Box::new(value))
    }
}

impl From<Box<EmbedMedia>> for BoxedEmbedMedia {
    fn from(value: Box<EmbedMedia>) -> Self {
        BoxedEmbedMedia(value)
    }
}

pub type UrlSignature = FixedStr<27>;

pub trait VisitMedia {
    fn visit_media<F>(&mut self, f: F)
    where
        F: FnMut(&mut BasicEmbedMedia);
}

impl VisitMedia for [BasicEmbedMedia] {
    fn visit_media<F>(&mut self, f: F)
    where
        F: FnMut(&mut BasicEmbedMedia),
    {
        self.iter_mut().for_each(f)
    }
}

impl<T> VisitMedia for Option<T>
where
    T: VisitMedia,
{
    fn visit_media<F>(&mut self, f: F)
    where
        F: FnMut(&mut BasicEmbedMedia),
    {
        if let Some(media) = self {
            media.visit_media(f);
        }
    }
}

impl VisitMedia for BoxedEmbedMedia {
    fn visit_media<F>(&mut self, f: F)
    where
        F: FnMut(&mut BasicEmbedMedia),
    {
        self.0.visit_media(f);
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct EmbedMedia {
    #[serde(flatten)]
    pub media: BasicEmbedMedia,

    #[serde(
        rename = "a",
        alias = "alternate",
        alias = "alts",
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "de::de_one_or_many"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub alts: Vec<BasicEmbedMedia>,
}

impl VisitMedia for EmbedMedia {
    fn visit_media<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut BasicEmbedMedia),
    {
        f(&mut self.media);
        self.alts.visit_media(f);
    }
}

mod de {
    use super::{BasicEmbedMedia, Vec};

    use serde::de::{Deserialize, Deserializer};

    pub fn de_one_or_many<'de, D>(deserializer: D) -> Result<Vec<BasicEmbedMedia>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum OneOrMany<T> {
            Many(Vec<T>),
            One(T),
        }

        OneOrMany::deserialize(deserializer).map(|v| match v {
            OneOrMany::Many(alts) => alts,
            OneOrMany::One(alt) => alloc::vec![alt],
        })
    }
}

#[cfg(feature = "rkyv")]
impl Deref for ArchivedEmbedMedia {
    type Target = ArchivedBasicEmbedMedia;

    fn deref(&self) -> &Self::Target {
        &self.media
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct BasicEmbedMedia {
    #[serde(rename = "u", alias = "url")]
    pub url: SmolStr,

    /// Non-visible description of the embedded media
    #[serde(
        rename = "d",
        alias = "description",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub description: Option<SmolStr>,

    /// Cryptographic signature for use with the proxy server
    #[serde(
        rename = "s",
        alias = "signature",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub signature: Option<FixedStr<27>>,

    /// height
    #[serde(
        rename = "h",
        alias = "height",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub height: Option<i32>,

    /// witdth
    #[serde(
        rename = "w",
        alias = "width",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub width: Option<i32>,

    #[serde(
        rename = "m",
        alias = "mime",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub mime: Option<SmolStr>,
}

impl Deref for EmbedMedia {
    type Target = BasicEmbedMedia;

    fn deref(&self) -> &BasicEmbedMedia {
        &self.media
    }
}

impl DerefMut for EmbedMedia {
    fn deref_mut(&mut self) -> &mut BasicEmbedMedia {
        &mut self.media
    }
}

impl EmbedMedia {
    /// If `this` is is empty, but the alternate field is not empty, set this to the alternate
    pub fn normalize(this: &mut EmbedMedia) {
        while this.url.is_empty() {
            let Some(alt) = this.alts.pop() else { break };

            let alts = core::mem::take(&mut this.alts);

            this.media = alt;
            this.alts = alts;
        }
    }

    pub fn is_empty(this: &Option<BoxedEmbedMedia>) -> bool {
        match this {
            Some(ref e) => e.url.is_empty(),
            None => true,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct EmbedProvider {
    #[serde(
        rename = "n",
        alias = "name",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub name: Option<SmolStr>,

    #[serde(
        rename = "u",
        alias = "url",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub url: Option<SmolStr>,

    #[serde(
        rename = "i",
        alias = "icon",
        default,
        skip_serializing_if = "EmbedMedia::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub icon: Option<BoxedEmbedMedia>,
}

impl EmbedProvider {
    pub fn is_none(&self) -> bool {
        is_none_or_empty(&self.name) && is_none_or_empty(&self.url) && EmbedMedia::is_empty(&self.icon)
    }
}

impl EmbedAuthor {
    pub fn is_none(this: &Option<Self>) -> bool {
        match this {
            Some(ref this) => {
                this.name.is_empty() && is_none_or_empty(&this.url) && EmbedMedia::is_empty(&this.icon)
            }
            None => true,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct EmbedAuthor {
    #[serde(rename = "n", alias = "name")]
    #[cfg_attr(feature = "typed-builder", builder(setter(into)))]
    pub name: SmolStr,

    #[serde(
        rename = "u",
        alias = "url",
        default,
        skip_serializing_if = "is_none_or_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    pub url: Option<SmolStr>,

    #[serde(
        rename = "i",
        alias = "icon",
        default,
        skip_serializing_if = "EmbedMedia::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub icon: Option<BoxedEmbedMedia>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "typed-builder", derive(typed_builder::TypedBuilder))]
#[cfg_attr(feature = "rkyv", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
#[cfg_attr(feature = "rkyv", archive(check_bytes))]
pub struct EmbedField {
    #[serde(
        rename = "n",
        alias = "name",
        default,
        skip_serializing_if = "SmolStr::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(setter(into)))]
    pub name: SmolStr,
    #[serde(
        rename = "v",
        alias = "value",
        default,
        skip_serializing_if = "SmolStr::is_empty"
    )]
    #[cfg_attr(feature = "typed-builder", builder(setter(into)))]
    pub value: SmolStr,

    #[serde(default, skip_serializing_if = "EmbedMedia::is_empty", alias = "image")]
    #[cfg_attr(feature = "typed-builder", builder(default, setter(into)))]
    #[cfg_attr(feature = "rkyv", with(NicheEmbedMedia))]
    pub img: Option<BoxedEmbedMedia>,

    /// Should use block-formatting
    #[serde(rename = "b", alias = "block", default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "typed-builder", builder(default))]
    pub block: bool,
}

impl EmbedField {
    pub fn is_empty(&self) -> bool {
        (self.name.is_empty() || self.value.is_empty()) && EmbedMedia::is_empty(&self.img)
    }
}

impl Default for EmbedV1 {
    #[inline]
    fn default() -> EmbedV1 {
        EmbedV1 {
            ts: Timestamp::UNIX_EPOCH,
            ty: EmbedType::Link,
            flags: EmbedFlags::empty(),
            url: None,
            canonical: None,
            title: None,
            description: None,
            color: None,
            author: None,
            provider: EmbedProvider::default(),
            img: None,
            audio: None,
            video: None,
            thumb: None,
            obj: None,
            fields: Vec::new(),
            footer: None,
        }
    }
}
