//! Hand-written bot lane notes (`lane_notes.toml`), keyed by Data Dragon id.

use model::LaneNote;
use std::collections::HashMap;

const NOTES: &str = include_str!("../lane_notes.toml");
/// A note is one short line on the champ select card.
pub const MAX_NOTE: usize = 80;

pub fn lane_notes() -> anyhow::Result<HashMap<String, LaneNote>> {
    Ok(toml::from_str(NOTES)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn notes_name_real_champions_and_stay_short() {
        let champs = crate::ddragon::parse_champions(include_str!("../../../fixtures/data/ddragon-champion.json")).unwrap();
        let ids: HashSet<_> = champs.values().map(|c| c.id.as_str()).collect();
        let notes = lane_notes().unwrap();
        assert!(notes.len() >= 40, "{}", notes.len());
        for (id, n) in &notes {
            assert!(ids.contains(id.as_str()), "lane_notes.toml: unknown champion {id}");
            assert!(!n.note.is_empty() && n.note.chars().count() <= MAX_NOTE, "{id}: {}", n.note);
        }
        let naut = &notes["Nautilus"];
        assert!(naut.level2 && naut.level6 && naut.engage && !naut.poke);
    }
}
