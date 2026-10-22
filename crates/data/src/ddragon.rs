//! Data Dragon items/champions + bundled `traits.toml` → [`StaticData`].

use model::StaticData;

/// Latest version, items and champions (cached per version).
pub async fn load(http: &crate::Http) -> anyhow::Result<StaticData> {
    let _ = http;
    todo!()
}
