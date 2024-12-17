use super::*;

#[derive(Default, Debug, serde::Deserialize)]
#[serde(default)]
pub struct WebAppManifest {
    pub name: Option<SmolStr>,
    pub name_localized: HashMap<SmolStr, LocalizedName>,

    pub short_name: Option<SmolStr>,
    pub short_name_localized: HashMap<SmolStr, LocalizedName>,

    pub description: Option<ThinString>,
    pub description_localized: HashMap<SmolStr, LocalizedDescription>,

    pub icons: Vec<ImageResource>,

    pub theme_color: Option<SmolStr>,
    pub background_color: Option<SmolStr>,
}

#[derive(Debug, serde::Deserialize)]
pub struct LocalizedName {
    pub value: SmolStr,
}

#[derive(Debug, serde::Deserialize)]
pub struct LocalizedDescription {
    pub value: ThinString,
}

mod de_localized {
    use serde::de::{self, Deserialize, Deserializer};
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

pub async fn try_fetch_manifest(
    state: &ServiceState,
    manifest_url: &str,
    params: &Params,
    embed: &mut EmbedV1,
) -> Result<(), Error> {
    let mut resp = retry_request(2, || state.client.get(manifest_url)).await?;

    if !resp.status().is_success() {
        return Err(Error::Failure(resp.status()));
    }

    let mut manifest: WebAppManifest = resp.json().await?;

    if embed.color.is_none() {
        embed.color = manifest
            .theme_color
            .or(manifest.background_color)
            .and_then(|c| crate::parser::embed::parse_color(&c))
    }

    if embed.description.is_none() {
        match manifest.description_localized.get(params.lang.as_deref().unwrap_or("en")) {
            Some(LocalizedDescription { value }) => embed.description = Some(value.as_str().into()),
            None => embed.description = manifest.description,
        }
    }

    // TODO: Use localized names
    if embed.provider.name.is_none() {
        embed.provider.name = manifest.name.or(manifest.short_name);
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
