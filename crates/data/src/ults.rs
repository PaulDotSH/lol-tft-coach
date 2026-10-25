//! Static R cooldowns from Data Dragon `champion/<Id>.json` (`spells[3]`).

use anyhow::Context;
use model_static::UltCooldowns;
use std::time::Duration;

const BASE: &str = "https://ddragon.leagueoflegends.com";

/// `id`: Data Dragon champion id ("MonkeyKing"). Cached per version.
pub async fn ult(http: &crate::Http, version: &str, id: &str) -> anyhow::Result<UltCooldowns> {
    let url = format!("{BASE}/cdn/{version}/data/en_US/champion/{id}.json");
    http.get_parsed(&url, Duration::from_secs(720 * 3600), parse_ult).await
}

pub fn parse_ult(json: &str) -> anyhow::Result<UltCooldowns> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let champ = v["data"].as_object().and_then(|d| d.values().next()).context("no champion in file")?;
    let r = &champ["spells"][3];
    let cooldown: Vec<f32> =
        r["cooldown"].as_array().context("no R cooldown")?.iter().filter_map(|c| c.as_f64()).map(|c| c as f32).collect();
    Ok(UltCooldowns { max_rank: r["maxrank"].as_u64().unwrap_or(cooldown.len() as u64) as u32, cooldown })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malphite_r() {
        let u = parse_ult(include_str!("../../../fixtures/data/ddragon-champion-malphite.json")).unwrap();
        assert_eq!(u, UltCooldowns { max_rank: 3, cooldown: vec![130.0, 105.0, 80.0] });
        assert!(parse_ult(r#"{"data":{}}"#).is_err());
    }
}
