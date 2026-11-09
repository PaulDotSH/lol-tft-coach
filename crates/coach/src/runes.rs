//! Champ select rune pages to choose from: against my lane opponent (Coachless, lolalytics'
//! matchup page), Coachless' best WPA page, lolalytics' most played and highest win rate pages.
//! Each has a row swapped when the enemy team calls for it and the champion's games back it up.

use crate::{common::*, consts::*, threat::Foe};
use model::*;

struct Swap {
    /// Runes sharing a row; any of them on the page may give way to `to`.
    row: &'static [RuneId],
    to: RuneId,
    /// Enemy names that call for it; empty = no swap.
    why: fn(&[Foe]) -> Vec<String>,
    label: &'static str,
}

const SWAPS: [Swap; 4] = [
    // Sorcery: Nullifying Orb / Manaflow Band / Nimbus Cloak.
    Swap { row: &[8224, 8226, 8275], to: 8224, why: magic_burst, label: "magic burst from" },
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

fn magic_burst(foes: &[Foe]) -> Vec<String> {
    names(foes, |f| f.magic >= RUNE_MAGIC && f.burst() >= ASSASSIN_FLAG_BURST, RUNE_THREATS)
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
fn stat(r: Option<&RuneData>, id: RuneId, slot: usize) -> Option<&RuneStat> {
    let s = r?.stats.get(&id)?;
    s.get(slot).or(s.first())
}

/// Swap `ids[i]` for a row-mate the enemies call for, if the champion's games (`r`) allow it.
fn adjust(r: Option<&RuneData>, ids: &[RuneId], slot: usize, foes: &[Foe]) -> Vec<(RuneId, Option<String>)> {
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

/// Rune pages against my lane opponent.
pub struct VsRunes<'a> {
    pub vs: ChampKey,
    /// Coachless filtered to the matchup (`popular` = its page).
    pub coachless: Option<&'a RuneData>,
    /// lolalytics matchup page.
    pub lolalytics: Option<&'a MatchupBuild>,
}

/// A page to offer and the per-rune stats its swaps are checked against.
struct Candidate<'a> {
    label: String,
    source: String,
    vs: Option<ChampKey>,
    page: &'a RunePage,
    stats: Option<&'a RuneData>,
}

fn option(sd: &StaticData, c: &Candidate, foes: &[Foe]) -> RuneOption {
    let picks = |v: Vec<(RuneId, Option<String>)>| v.into_iter().map(|(id, why)| pick(sd, id, why)).collect();
    let p = c.page;
    RuneOption {
        label: c.label.clone(),
        source: c.source.clone(),
        vs: c.vs,
        primary: picks(adjust(c.stats, &p.primary, 0, foes)),
        secondary: picks(adjust(c.stats, &p.secondary, 1, foes)),
        // Only the third shard row holds Tenacity; Health Scaling also sits in the second.
        shards: picks(match p.shards.split_at_checked(2) {
            Some((head, tail)) => head.iter().map(|&id| (id, None)).chain(adjust(c.stats, tail, 0, foes)).collect(),
            None => adjust(c.stats, &p.shards, 0, foes),
        }),
        win_rate: p.win_rate,
        games: p.games,
        also: vec![],
    }
}

/// Same runes in the same rows (the secondary tree's two in either order).
fn same(a: &RuneOption, b: &RuneOption) -> bool {
    let ids = |v: &[RunePick]| v.iter().map(|r| r.id).collect::<Vec<_>>();
    let sec = |o: &RuneOption| {
        let mut v = ids(&o.secondary);
        v.sort_unstable();
        v
    };
    ids(&a.primary) == ids(&b.primary) && sec(a) == sec(b) && ids(&a.shards) == ids(&b.shards)
}

pub fn advice(sd: &StaticData, stats: &ChampStats, foes: &[Foe], vs: Option<&VsRunes>) -> Option<RuneAdvice> {
    let lola = stats.runes.as_ref();
    let lola_src = lolalytics_source(stats);
    let cl_src = stats.coachless.as_deref().map_or_else(|| "Coachless".to_owned(), |p| format!("Coachless {p}"));
    let mut cands: Vec<Candidate> = vec![];
    if let Some(v) = vs {
        let label = format!("vs {}", sd.champs.get(&v.vs).map_or("lane opponent", |c| c.name.as_str()));
        let c = |source: &str, page, stats| Candidate { label: label.clone(), source: source.to_owned(), vs: Some(v.vs), page, stats };
        if let Some(d) = v.coachless.filter(|d| d.popular.games >= RUNE_VS_MIN_GAMES) {
            cands.push(c(&cl_src, &d.popular, Some(d)));
        }
        let lola_vs = v.lolalytics.filter(|m| m.vs == v.vs).and_then(|m| m.rune_page.as_ref());
        if let Some(p) = lola_vs.filter(|p| p.games >= RUNE_VS_MIN_GAMES) {
            cands.push(c(&lola_src, p, lola));
        }
    }
    let own = |label: &str, source: &str, page, stats| Candidate { label: label.to_owned(), source: source.to_owned(), vs: None, page, stats };
    if let Some(d) = &stats.coachless_runes {
        cands.push(own("Best WPA", &cl_src, &d.popular, Some(d)));
    }
    if let Some(r) = lola {
        cands.push(own("Most played", &lola_src, &r.popular, lola));
        if r.best.games >= RUNE_ALT_MIN_GAMES {
            cands.push(own("Highest win rate", &lola_src, &r.best, lola));
        }
    }
    let mut options: Vec<RuneOption> = vec![];
    for c in &cands {
        let o = option(sd, c, foes);
        match options.iter_mut().find(|x| same(x, &o)) {
            Some(x) => x.also.push(format!("{} · {}", o.label, o.source)),
            None => options.push(o),
        }
    }
    if options.is_empty() {
        return None;
    }
    Some(RuneAdvice { options, summoners: stats.summoners.iter().filter_map(|&id| spell_name(id)).map(str::to_owned).collect() })
}

fn spell_name(id: u32) -> Option<&'static str> {
    Some(match id {
        1 => "Cleanse",
        3 => "Exhaust",
        4 => "Flash",
        6 => "Ghost",
        7 => "Heal",
        11 => "Smite",
        12 => "Teleport",
        14 => "Ignite",
        21 => "Barrier",
        _ => return None,
    })
}
