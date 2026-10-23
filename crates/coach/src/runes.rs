//! Champ select rune page: lolalytics' most played page, with a row swapped
//! when the enemy team calls for it and the champion's own games back it up.

use crate::{common::fmt_games, consts::*, threat::Foe};
use model::*;

struct Swap {
    /// Runes sharing a row; any of them on the page may give way to `to`.
    row: &'static [RuneId],
    to: RuneId,
    /// Enemy names that call for it; empty = no swap.
    why: fn(&[Foe]) -> Vec<String>,
    label: &'static str,
}

const SWAPS: [Swap; 3] = [
    // Resolve: Conditioning / Second Wind / Bone Plating.
    Swap { row: &[8429, 8444, 8473], to: 8473, why: bursty, label: "burst from" },
    // Precision: Coup de Grace / Cut Down / Last Stand.
    Swap { row: &[8014, 8017, 8299], to: 8017, why: tanks, label: "tanks:" },
    // Third shard: Health / Tenacity / Health Scaling.
    Swap { row: &[5011, 5013, 5001], to: 5013, why: hard_cc, label: "CC from" },
];

fn names(foes: &[Foe], hit: impl Fn(&Foe) -> bool, min: usize) -> Vec<String> {
    let v: Vec<String> = foes.iter().filter(|f| hit(f)).map(|f| f.champ.name.clone()).collect();
    if v.len() >= min { v } else { vec![] }
}

fn bursty(foes: &[Foe]) -> Vec<String> {
    names(foes, |f| f.burst() >= ASSASSIN_FLAG_BURST, RUNE_THREATS)
}

fn tanks(foes: &[Foe]) -> Vec<String> {
    names(foes, |f| f.has(ChampClass::Tank) || f.armor_tank().max(f.mr_tank()) >= FLAG, RUNE_THREATS)
}

fn hard_cc(foes: &[Foe]) -> Vec<String> {
    names(foes, |f| f.champ.traits.cc >= RUNE_HARD_CC, RUNE_CC_FOES)
}

fn pick(sd: &StaticData, id: RuneId, reason: Option<String>) -> RunePick {
    let (name, icon) = sd.runes.get(&id).map_or_else(|| (format!("Rune {id}"), String::new()), |r| (r.name.clone(), r.icon.clone()));
    RunePick { id, name, icon, reason }
}

/// `slot`: 0 = primary tree, 1 = secondary (shards have only 0).
fn stat(r: &RuneData, id: RuneId, slot: usize) -> Option<&RuneStat> {
    let s = r.stats.get(&id)?;
    s.get(slot).or(s.first())
}

/// Swap `ids[i]` for a row-mate the enemies call for, if the champion's games allow it.
fn adjust(r: &RuneData, ids: &[RuneId], slot: usize, foes: &[Foe]) -> Vec<(RuneId, Option<String>)> {
    let mut out: Vec<(RuneId, Option<String>)> = ids.iter().map(|&id| (id, None)).collect();
    for s in &SWAPS {
        let Some(i) = out.iter().position(|(id, _)| s.row.contains(id) && *id != s.to) else { continue };
        if ids.contains(&s.to) {
            continue;
        }
        let who = (s.why)(foes);
        let (Some(new), Some(old)) = (stat(r, s.to, slot), stat(r, out[i].0, slot)) else { continue };
        if who.is_empty() || new.games < RUNE_MIN_GAMES || new.win_rate < old.win_rate - RUNE_WR_SLACK {
            continue;
        }
        out[i] = (s.to, Some(format!("{} {} ({:.1}% over {} games)", s.label, who.join(", "), new.win_rate, fmt_games(new.games as u64))));
    }
    out
}

pub fn advice(sd: &StaticData, stats: &ChampStats, foes: &[Foe]) -> Option<RuneAdvice> {
    let r = stats.runes.as_ref()?;
    let p = &r.popular;
    let picks = |v: Vec<(RuneId, Option<String>)>| v.into_iter().map(|(id, why)| pick(sd, id, why)).collect();
    let alt = Some(&r.best)
        .filter(|b| b.primary.first() != p.primary.first() && b.games >= RUNE_ALT_MIN_GAMES)
        .and_then(|b| Some(RuneAlt { keystone: pick(sd, *b.primary.first()?, None), win_rate: b.win_rate, games: b.games }));
    Some(RuneAdvice {
        primary: picks(adjust(r, &p.primary, 0, foes)),
        secondary: picks(adjust(r, &p.secondary, 1, foes)),
        // Only the third shard row holds Tenacity; Health Scaling also sits in the second.
        shards: picks(match p.shards.split_at_checked(2) {
            Some((head, tail)) => head.iter().map(|&id| (id, None)).chain(adjust(r, tail, 0, foes)).collect(),
            None => adjust(r, &p.shards, 0, foes),
        }),
        win_rate: p.win_rate,
        games: p.games,
        alt,
    })
}
