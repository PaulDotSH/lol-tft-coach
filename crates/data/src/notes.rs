//! Hand-written laning notes, keyed by Data Dragon id: one per champion
//! (`champ_notes.toml`), a few per pairing (`matchup_notes.toml`) and each champion's kit
//! tags (`kits.toml`).

use model_static::{ChampNote, LaneNotes, PairTip};
use serde::Deserialize;
use std::collections::HashMap;

const CHAMPS: &str = include_str!("../champ_notes.toml");
const PAIRS: &str = include_str!("../matchup_notes.toml");
const KITS: &str = include_str!("../kits.toml");
/// The tags `kits.toml` may use.
pub const KIT_TAGS: [&str; 21] = [
    "poke", "autos", "burst", "shield", "sustain", "armor_stack", "spell_shield", "dodge", "auto_block", "proj_block", "as_slow", "hook",
    "bind", "zone_cc", "point_cc", "engage", "dash", "blink", "anti_dash", "true_dmg", "pct_hp",
];
/// Each note line is one short line on the matchup card.
pub const MAX_LINE: usize = 90;

/// A pairing in `matchup_notes.toml`: one line, or a list of tips.
#[derive(Deserialize)]
#[serde(untagged)]
enum Pair {
    One(String),
    Many(Vec<Tip>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Tip {
    Text(String),
    Keystone { keystone: String, tip: String },
}

pub fn lane_notes() -> anyhow::Result<LaneNotes> {
    let champs: HashMap<String, ChampNote> = toml::from_str(CHAMPS)?;
    let pairs: HashMap<String, HashMap<String, Pair>> = toml::from_str(PAIRS)?;
    let tips = |p: Pair| match p {
        Pair::One(text) => vec![PairTip { text, keystone: None }],
        Pair::Many(tips) => tips
            .into_iter()
            .map(|t| match t {
                Tip::Text(text) => PairTip { text, keystone: None },
                Tip::Keystone { keystone, tip } => PairTip { text: tip, keystone: Some(keystone) },
            })
            .collect(),
    };
    let pairs = pairs.into_iter().map(|(me, them)| (me, them.into_iter().map(|(t, p)| (t, tips(p))).collect())).collect();
    Ok(LaneNotes { champs, pairs, kits: toml::from_str(KITS)? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_champion_has_short_notes_and_real_spell_keys() {
        let champs = crate::ddragon::parse_champions(include_str!("../../../fixtures/data/ddragon-champion.json")).unwrap();
        let ids: HashSet<_> = champs.values().map(|c| c.id.as_str()).collect();
        let notes = lane_notes().unwrap();
        for id in &ids {
            assert!(notes.champs.contains_key(*id), "champ_notes.toml: no entry for {id}");
        }
        for (id, n) in &notes.champs {
            assert!(ids.contains(id.as_str()), "champ_notes.toml: unknown champion {id}");
            for (field, line) in [("wins", &n.wins), ("spike", &n.spike), ("trade", &n.trade)] {
                assert!(!line.is_empty() && line.chars().count() <= MAX_LINE, "{id}.{field}: {line}");
            }
            assert!(!n.watch.is_empty() && n.watch.chars().all(|c| "QWER".contains(c)), "{id}.watch: {}", n.watch);
        }
        let runes = crate::ddragon::parse_runes(include_str!("../../../fixtures/data/ddragon-runes.json")).unwrap();
        for (me, them) in &notes.pairs {
            assert!(ids.contains(me.as_str()), "matchup_notes.toml: unknown champion {me}");
            for (t, tips) in them {
                assert!(ids.contains(t.as_str()), "matchup_notes.toml: unknown champion {t}");
                assert!(tips.first().is_some_and(|tip| tip.keystone.is_none()), "{me} vs {t}: the first tip is for every keystone");
                for tip in tips {
                    assert!(!tip.text.is_empty() && tip.text.chars().count() <= 2 * MAX_LINE, "{me} vs {t}: {}", tip.text);
                    assert!(tip.keystone.as_ref().is_none_or(|k| runes.values().any(|r| &r.name == k)), "{me} vs {t}: no keystone {:?}", tip.keystone);
                }
            }
        }
        for (id, kit) in &notes.kits {
            assert!(ids.contains(id.as_str()), "kits.toml: unknown champion {id}");
            for (tag, key) in kit {
                assert!(KIT_TAGS.contains(&tag.as_str()), "kits.toml: {id} has unknown tag {tag}");
                assert!(["P", "Q", "W", "E", "R"].contains(&key.as_str()), "kits.toml: {id}.{tag} = {key}");
            }
        }
        assert_eq!(notes.kits["Malphite"]["as_slow"], "E");
        let naut = &notes.champs["Nautilus"];
        assert!(naut.level2 && naut.level6 && naut.engage && !naut.poke);
        assert!(!notes.pair("Sett", "Zoe").is_empty() && notes.pair("Zoe", "Sett").is_empty());
        let sett = notes.pair("Malphite", "Sett");
        assert!(sett.len() > 2 && sett.iter().any(|t| t.keystone.as_deref() == Some("Grasp of the Undying")), "{sett:?}");
    }
}
