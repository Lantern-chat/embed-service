use scraper::Selector;

#[derive(Debug, Clone)]
pub struct FieldSelector {
    pub selector: Selector,
    pub attribute: Option<String>,
}

macro_rules! decl_site_field_selectors {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$field_meta:meta])* $field:ident,)*
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $( $(#[$field_meta])* pub $field: Option<FieldSelector>,)*
        }

        impl $name {
            pub const fn is_empty(&self) -> bool {
                $(self.$field.is_none() &&)* true
            }

            // paste::paste! {$(
            //     #[inline]
            //     pub fn [<extract_ $field>](&self) -> Option<ThinString> {
            //         match self.$field {
            //             Some(ref selector) => selector.extract(doc),
            //             None => None,
            //         }
            //     }
            // )*}
        }
    };
}

decl_site_field_selectors! {
    /// CSS Selectors for extracting metadata from a site
    #[derive(Default, Debug, Clone, serde::Deserialize)]
    #[serde(default)]
    pub struct SiteFieldSelectors {
        /// CSS selector for the title of the page
        title,
        /// CSS selector for the description of the page
        description,
        /// CSS selector for the primary image of the page
        image_url,
        /// CSS selector for the alt text of the primary image
        image_alt,
        /// CSS selector for the width of the primary image
        image_width,
        /// CSS selector for the height of the primary image
        image_height,
        /// CSS selector for the author of the page
        author_name,
        /// CSS selector for the URL of the author
        author_url,
        /// CSS selector for the icon of the author
        author_icon,
        /// CSS selector for the alt text of the author icon
        author_icon_alt,
        author_icon_width,
        author_icon_height,

        provider_name,
        provider_url,
        provider_icon,
        provider_icon_alt,
        provider_icon_width,
        provider_icon_height,
    }
}

use serde::de::{self, Deserialize, Deserializer};

impl<'de> Deserialize<'de> for FieldSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        return deserializer.deserialize_str(Visitor);

        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = FieldSelector;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "A valid CSS selector with optional attribute specified by `< attribute` at the end",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let attribute = match v.split('<').skip(1).last() {
                    Some(attr) if !attr.contains(['\'', '"']) => Some(attr),
                    _ => None,
                };

                let selector = match attribute {
                    Some(attr) => &v[..v.len() - attr.len() - 1],
                    None => v,
                };

                let selector = Selector::parse(selector.trim()).map_err(E::custom)?;
                let attribute = attribute.map(|attr| attr.trim().to_owned());

                Ok(FieldSelector { selector, attribute })
            }
        }
    }
}

use ::embed::thin_str::ThinString;

impl FieldSelector {
    pub fn extract(&self, doc: &scraper::Html) -> Option<ThinString> {
        let mut out = ThinString::new();

        for node in doc.select(&self.selector) {
            match &self.attribute {
                Some(attr) => {
                    if let Some(value) = node.value().attr(attr) {
                        out.push_str(value);
                    }
                }
                None => out.extend(node.text()),
            }
        }

        (!out.is_empty()).then_some(out)
    }
}
