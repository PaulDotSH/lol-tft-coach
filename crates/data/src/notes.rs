//! Hand-written laning notes, keyed by Data Dragon id: one per champion
//! (`champ_notes.toml`) and a few per pairing (`matchup_notes.toml`).

use model_static::{ChampNote, LaneNotes};
use std::collections::HashMap;

const CHAMPS: &str = include_str!("../champ_notes.toml");
const PAIRS: &str = include_str!("../matchup_notes.toml");
/// Each note line is one short line on the matchup card.
pub const MAX_LINE: usize = 90;

pub fn lane_notes() -> anyhow::Result<LaneNotes> {
    let champs: HashMap<String, ChampNote> = toml::from_str(CHAMPS)?;
    let pairs: HashMap<String, HashMap<String, String>> = toml::from_str(PAIRS)?;
    Ok(LaneNotes { champs, pairs })
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
        for (me, them) in &notes.pairs {
            assert!(ids.contains(me.as_str()), "matchup_notes.toml: unknown champion {me}");
            for (t, line) in them {
                assert!(ids.contains(t.as_str()), "matchup_notes.toml: unknown champion {t}");
                assert!(!line.is_empty() && line.chars().count() <= 2 * MAX_LINE, "{me} vs {t}: {line}");
            }
        }
        let naut = &notes.champs["Nautilus"];
        assert!(naut.level2 && naut.level6 && naut.engage && !naut.poke);
        assert!(notes.pair("Sett", "Zoe").is_some());
    }
}
