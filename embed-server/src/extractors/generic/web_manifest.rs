use super::*;

#[derive(Default, Debug, serde::Deserialize)]
#[serde(default)]
pub struct WebAppManifest {
    pub name: Option<SmolStr>,
    pub name_localized: HashMap<SmolStr, LocalizedValue<SmolStr>>,

    pub short_name: Option<SmolStr>,
    pub short_name_localized: HashMap<SmolStr, LocalizedValue<SmolStr>>,

    pub description: Option<ThinString>,
    pub description_localized: HashMap<SmolStr, LocalizedValue<ThinString>>,

    pub icons: Vec<ImageResource>,

    pub theme_color: Option<SmolStr>,
    pub background_color: Option<SmolStr>,
}

fn get_localized<T>(
    lang: Option<&str>,
    generic: Option<T>,
    localized: &HashMap<SmolStr, LocalizedValue<T>>,
) -> Option<T>
where
    T: Clone,
{
    if let Some(lang) = lang {
        if let Some(value) = localized.get(lang) {
            return Some(value.value.clone());
        }

        if let Some((lang, _)) = lang.split_once('-') {
            if let Some(value) = localized.get(lang) {
                return Some(value.value.clone());
            }
        }
    }

    generic
}

impl WebAppManifest {
    pub fn get_name(&mut self, lang: Option<&str>) -> Option<SmolStr> {
        get_localized(lang, self.name.take(), &self.name_localized)
    }

    pub fn get_short_name(&mut self, lang: Option<&str>) -> Option<SmolStr> {
        get_localized(lang, self.short_name.take(), &self.short_name_localized)
    }

    pub fn get_description(&mut self, lang: Option<&str>) -> Option<ThinString> {
        get_localized(lang, self.description.take(), &self.description_localized)
    }
}

#[derive(Debug)]
pub struct LocalizedValue<T> {
    pub value: T,
}

mod de_localized {
    use super::LocalizedValue;
    use serde::de::{self, Deserialize, Deserializer};

    impl<'de, T> Deserialize<'de> for LocalizedValue<T>
    where
        T: for<'a> From<&'a str>,
        T: Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct Visitor<T>(std::marker::PhantomData<T>);

            impl<'de, T> de::Visitor<'de> for Visitor<T>
            where
                T: for<'a> From<&'a str>,
                T: Deserialize<'de>,
            {
                type Value = LocalizedValue<T>;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(f, "a localized value or map containing a 'value' field")
                }

                fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: de::MapAccess<'de>,
                {
                    let mut value = None;

                    while let Some(key) = map.next_key::<String>()? {
                        if key == "value" {
                            value = Some(map.next_value()?);
                        } else {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }

                    Ok(LocalizedValue {
                        value: value.ok_or_else(|| de::Error::missing_field("value"))?,
                    })
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(LocalizedValue { value: T::from(v) })
                }
            }

            deserializer.deserialize_any(Visitor(std::marker::PhantomData))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ImageResource {
    pub src: String,

    #[serde(default)]
    pub sizes: Option<SmolStr>,

    #[serde(default, rename = "type")]
    pub mime: Option<SmolStr>,

    #[serde(default)]
    pub purpose: Option<String>,
}

pub fn needs_manifest(embed: &EmbedV1) -> bool {
    embed.provider.name.is_none() || embed.description.is_none() || embed.provider.icon.is_none()
}

pub async fn try_fetch_manifest(
    state: &ServiceState,
    base_url: &Url,
    manifest_url: &str,
    params: &Params,
    embed: &mut EmbedV1,
) -> Result<(), Error> {
    // if the path can't be resolved, don't bother fetching
    let Ok(manifest_url) = crate::parser::quirks::resolve_relative_url(base_url, manifest_url) else {
        return Ok(());
    };

    let mut resp = state.client.get(manifest_url).send().await?;

    if !resp.status().is_success() {
        return Err(Error::Failure(resp.status()));
    }

    let mut manifest: WebAppManifest = resp.json().await?;

    let lang = params.lang.as_deref().unwrap_or("en");

    if embed.description.is_none() {
        embed.description = manifest.get_description(Some(lang));
    }

    if embed.provider.name.is_none() {
        embed.provider.name = manifest.get_name(Some(lang)).or_else(|| manifest.get_short_name(Some(lang)));
    }

    if embed.color.is_none() {
        embed.color = manifest
            .theme_color
            .or(manifest.background_color)
            .and_then(|c| crate::parser::embed::parse_color(&c))
    }

    if embed.provider.icon.is_none() {
        if let Some(icon) = manifest.icons.first_mut() {
            let mut media = Box::<EmbedMedia>::default().with_url(icon.src.clone());

            media.mime = icon.mime.take();

            if let Some(ref sizes) = icon.sizes {
                for size in sizes.split(' ') {
                    let Some((width, height)) = size.split_once('x') else {
                        continue;
                    };

                    media.width = width.parse().ok();
                    media.height = height.parse().ok();

                    // if the icon is small enough, break the loop
                    if matches!((media.width, media.height), (Some(w), Some(h)) if w <= 512 || h <= 512) {
                        break;
                    }
                }
            }

            embed.provider.icon = Some(media);
        }
    }

    Ok(())
}
