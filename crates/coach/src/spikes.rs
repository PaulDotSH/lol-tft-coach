//! Enemy power spikes from visible information: R rank by level with its
//! static base cooldown (Q5) and finished legendaries (V6). Never tracks when
//! an ability was used.

use crate::{consts::*, tracker::{Tracker, is_legendary}};
use model::*;
use std::collections::HashMap;

pub fn legendaries(sd: &StaticData, items: &[ItemId]) -> u32 {
    items.iter().filter(|i| sd.items.get(i).is_some_and(is_legendary)).count() as u32
}

/// None before the first rank, or for ults that do not rank with level (Udyr).
pub fn ult(level: u32, data: Option<&UltCooldowns>) -> Option<UltInfo> {
    let levels: &[u32] = match data.map_or(3, |d| d.max_rank) {
        3 => &ULT_LEVELS,
        4 => &ULT_LEVELS_4,
        _ => return None,
    };
    let rank = levels.iter().filter(|&&l| level >= l).count() as u32;
    (rank > 0).then(|| UltInfo {
        rank,
        cooldowns: data.map_or(vec![], |d| d.cooldown.clone()),
        spike: levels.contains(&level),
    })
}

/// Fill each threat's R info and newly finished items.
pub fn annotate(threats: &mut [Threat], snap: &GameSnapshot, tracker: &Tracker, ults: &HashMap<ChampKey, UltCooldowns>) {
    let Some(me) = snap.players.get(snap.me) else { return };
    for t in threats {
        let Some((i, p)) = snap.players.iter().enumerate().find(|(_, p)| p.team != me.team && p.champ == t.champ) else {
            continue;
        };
        t.ult = ult(p.level, ults.get(&p.champ));
        t.new_items = tracker.new_items(i, &p.items, snap.time);
    }
}
