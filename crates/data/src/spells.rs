//! Static spell cooldowns from Data Dragon `champion/<Id>.json`: all four spells
//! for the matchup card, R alone for enemy ult ranks.

use anyhow::Context;
use model_static::{SpellInfo, UltCooldowns};
use serde_json::Value;
use std::time::Duration;

const BASE: &str = "https://ddragon.leagueoflegends.com";
const TTL: Duration = Duration::from_secs(720 * 3600);

fn url(version: &str, id: &str) -> String {
    format!("{BASE}/cdn/{version}/data/en_US/champion/{id}.json")
}

/// `id`: Data Dragon champion id ("MonkeyKing"). Cached per version.
pub async fn ult(http: &crate::Http, version: &str, id: &str) -> anyhow::Result<UltCooldowns> {
    http.get_parsed(&url(version, id), TTL, parse_ult).await
}

/// Q W E R, same file as [`ult`].
pub async fn spells(http: &crate::Http, version: &str, id: &str) -> anyhow::Result<Vec<SpellInfo>> {
    http.get_parsed(&url(version, id), TTL, parse_spells).await
}

fn champ(json: &str) -> anyhow::Result<Value> {
    let mut v: Value = serde_json::from_str(json)?;
    let d = v["data"].as_object_mut().context("no champion data")?;
    let key = d.keys().next().cloned().context("no champion in file")?;
    Ok(d.remove(&key).unwrap_or_default())
}

fn cooldowns(s: &Value) -> Option<Vec<f32>> {
    Some(s["cooldown"].as_array()?.iter().filter_map(|c| c.as_f64()).map(|c| c as f32).collect())
}

pub fn parse_ult(json: &str) -> anyhow::Result<UltCooldowns> {
    let c = champ(json)?;
    let r = &c["spells"][3];
    let cooldown = cooldowns(r).context("no R cooldown")?;
    Ok(UltCooldowns { max_rank: r["maxrank"].as_u64().unwrap_or(cooldown.len() as u64) as u32, cooldown })
}

pub fn parse_spells(json: &str) -> anyhow::Result<Vec<SpellInfo>> {
    let c = champ(json)?;
    let spells = c["spells"].as_array().filter(|s| s.len() == 4).context("expected four spells")?;
    spells
        .iter()
        .map(|s| {
            let id = s["id"].as_str().unwrap_or_default();
            Ok(SpellInfo {
                name: s["name"].as_str().context("spell without a name")?.to_owned(),
                icon: s["image"]["full"].as_str().map_or_else(|| format!("{id}.png"), str::to_owned),
                cooldown: cooldowns(s).context("spell without cooldowns")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MALPHITE: &str = include_str!("../../../fixtures/data/ddragon-champion-malphite.json");

    #[test]
    fn malphite_r() {
        let u = parse_ult(MALPHITE).unwrap();
        assert_eq!(u, UltCooldowns { max_rank: 3, cooldown: vec![130.0, 105.0, 80.0] });
        assert!(parse_ult(r#"{"data":{}}"#).is_err());
    }

    #[test]
    fn malphite_spells() {
        let s = parse_spells(MALPHITE).unwrap();
        let names: Vec<&str> = s.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Seismic Shard", "Thunderclap", "Ground Slam", "Unstoppable Force"]);
        assert_eq!((s[1].icon.as_str(), s[1].cooldown.as_slice()), ("Obduracy.png", [12.0, 11.5, 11.0, 10.5, 10.0].as_slice()));
        assert!(parse_spells(r#"{"data":{"X":{"spells":[]}}}"#).is_err());
    }
}
