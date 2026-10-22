//! Enemy threat scores, damage labels and flags.

use crate::{common::*, consts::*};
use model::*;

/// One enemy as the needs/items code sees it.
pub struct Foe<'a> {
    pub champ: &'a Champion,
    /// 0..=100, personal weights applied.
    pub score: f32,
    /// Effective magic share of their damage (traits shifted by items).
    pub magic: f32,
    pub kda: Option<(u32, u32, u32)>,
    pub kills_on_me: u32,
    pub inv: Inv,
}

impl Foe<'_> {
    pub fn has(&self, c: ChampClass) -> bool {
        self.champ.classes.contains(&c)
    }

    pub fn burst(&self) -> f32 {
        let a = if self.has(ChampClass::Assassin) { ASSASSIN_BURST } else { 0.0 };
        self.champ.traits.burst.max(a)
    }

    pub fn armor_tank(&self) -> f32 {
        let t = self.champ.traits.tank * TANK_TRAIT;
        clamp01(t + self.inv.armor / TANK_RESIST_FULL + self.inv.hp / TANK_HP_FULL)
    }

    pub fn mr_tank(&self) -> f32 {
        let t = self.champ.traits.tank * TANK_TRAIT;
        clamp01(t + self.inv.mr / TANK_RESIST_FULL + self.inv.hp / TANK_HP_FULL)
    }

    pub fn crit(&self) -> f32 {
        let m = if self.has(ChampClass::Marksman) { MARKSMAN_CRIT * (1.0 - self.champ.traits.on_hit) } else { 0.0 };
        clamp01(self.inv.crit / CRIT_FULL).max(m)
    }

    pub fn attack_speed(&self) -> f32 {
        let m = if self.has(ChampClass::Marksman) { MARKSMAN_AS } else { 0.0 };
        clamp01(self.inv.attack_speed / AS_FULL).max(m).max(self.champ.traits.on_hit)
    }

    /// "Zed 7/1, killed you 3×" / "Zed 7/1/2" / "Zed".
    pub fn label(&self) -> String {
        let n = &self.champ.name;
        match self.kda {
            Some((k, d, _)) if self.kills_on_me > 0 => format!("{n} {k}/{d}, killed you {}×", self.kills_on_me),
            Some((k, d, a)) => format!("{n} {k}/{d}/{a}"),
            None => n.clone(),
        }
    }

    pub fn damage_label(&self) -> &'static str {
        if self.champ.traits.true_damage >= TRUE_LABEL {
            "True"
        } else if self.magic >= AP_LABEL {
            "AP"
        } else if self.magic <= AD_LABEL {
            "AD"
        } else {
            "Mixed"
        }
    }

    pub fn flags(&self) -> Vec<String> {
        let t = &self.champ.traits;
        [
            ("assassin", self.has(ChampClass::Assassin) || t.burst >= FLAG),
            ("heals", t.heals >= FLAG),
            ("cc", t.cc >= FLAG),
            ("tank", self.has(ChampClass::Tank) || self.armor_tank().max(self.mr_tank()) >= FLAG),
            ("crit", self.inv.crit >= CRIT_FLAG),
            ("on-hit", t.on_hit >= FLAG),
            ("true", t.true_damage >= FLAG),
        ]
        .into_iter()
        .filter(|f| f.1)
        .map(|f| f.0.to_string())
        .collect()
    }
}

/// Trait magic share, shifted toward what their items say.
pub fn magic_share(c: &Champion, inv: &Inv) -> f32 {
    let (ap, ad) = (inv.ap, inv.ad * AD_TO_AP);
    if ap + ad <= 0.0 {
        return c.traits.magic_share;
    }
    let w = ITEM_SHARE_MAX * (inv.dmg_gold as f32 / ITEM_SHARE_FULL_GOLD).min(1.0);
    (1.0 - w) * c.traits.magic_share + w * ap / (ap + ad)
}

/// Enemy known only by champion (champ select): average threat, no items.
pub fn generic_foe(c: &Champion) -> Foe<'_> {
    Foe {
        champ: c,
        score: AVG_THREAT,
        magic: c.traits.magic_share,
        kda: None,
        kills_on_me: 0,
        inv: Inv::default(),
    }
}

pub fn kills_by(snap: &GameSnapshot, killer: usize, victim: usize) -> u32 {
    snap.events
        .iter()
        .filter(|e| matches!(e, GameEvent::Kill { killer: Some(k), victim: v, .. } if *k == killer && *v == victim))
        .count() as u32
}

/// Same `position` as me; else enemy whose lolalytics default lane is mine. None outside the Rift.
pub fn lane_opponent(snap: &GameSnapshot, stats: Option<&ChampStats>) -> Option<usize> {
    let me = snap.players.get(snap.me)?;
    if snap.mode != Mode::Rift {
        return None;
    }
    let lane = me.position.or(stats.and_then(|s| s.lane))?;
    let enemies = || snap.players.iter().enumerate().filter(|(_, p)| p.team != me.team);
    if let Some((i, _)) = enemies().find(|(_, p)| p.position == Some(lane)) {
        return Some(i);
    }
    let counters = &stats?.counters;
    enemies()
        .filter(|(_, p)| p.position.is_none())
        .filter_map(|(i, p)| {
            counters.iter().find(|m| m.champ == p.champ && m.default_lane == Some(lane)).map(|m| (i, m.games))
        })
        .max_by_key(|&(_, g)| g)
        .map(|(i, _)| i)
}

pub struct Assessed<'a> {
    /// Sorted by score, highest first; same order as `threats`.
    pub foes: Vec<Foe<'a>>,
    pub threats: Vec<Threat>,
    pub lane_opp: Option<usize>,
}

fn cs_group(p: &Player, mode: Mode) -> usize {
    match (mode, p.position) {
        (Mode::Rift, Some(Lane::Jungle)) => 1,
        (Mode::Rift, Some(Lane::Support)) => 2,
        _ => 0,
    }
}

pub fn assess<'a>(snap: &GameSnapshot, sd: &'a StaticData, stats: Option<&ChampStats>) -> Assessed<'a> {
    let Some(me) = snap.players.get(snap.me) else {
        return Assessed { foes: vec![], threats: vec![], lane_opp: None };
    };
    let invs: Vec<Inv> = snap.players.iter().map(|p| inventory(sd, &p.items)).collect();
    let n = snap.players.len().max(1) as f32;
    let kda = |p: &Player| p.kills as f32 + ASSIST_WEIGHT * p.assists as f32 - p.deaths as f32;
    let avg_gold = invs.iter().map(|v| v.gold as f32).sum::<f32>() / n;
    let avg_kda = snap.players.iter().map(kda).sum::<f32>() / n;
    let avg_lvl = snap.players.iter().map(|p| p.level as f32).sum::<f32>() / n;
    let (mut cs_sum, mut cs_n) = ([0f32; 3], [0f32; 3]);
    for p in &snap.players {
        let g = cs_group(p, snap.mode);
        cs_sum[g] += p.cs as f32;
        cs_n[g] += 1.0;
    }
    let lane_opp = lane_opponent(snap, stats);

    let mut rows: Vec<(Foe, Threat)> = snap
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.team != me.team)
        .map(|(i, p)| {
            let inv = invs[i];
            let g = cs_group(p, snap.mode);
            let avg_cs = cs_sum[g] / cs_n[g].max(1.0);
            let z = W_GOLD * (inv.gold as f32 - avg_gold) / avg_gold.max(GOLD_FLOOR)
                + W_KDA * (kda(p) - avg_kda)
                + W_LEVEL * (p.level as f32 - avg_lvl)
                + W_CS * (p.cs as f32 - avg_cs) / avg_cs.max(CS_FLOOR);
            let kom = kills_by(snap, i, snap.me);
            let is_opp = lane_opp == Some(i);
            let mult = (1.0 + KILLS_ON_ME_STEP * kom.min(KILLS_ON_ME_CAP) as f32) * if is_opp { LANE_OPP_MULT } else { 1.0 };
            let score = (50.0 * (1.0 + z.tanh()) * mult).min(100.0);
            let c = champ(sd, p.champ);
            let foe = Foe {
                champ: c,
                score,
                magic: magic_share(c, &inv),
                kda: Some((p.kills, p.deaths, p.assists)),
                kills_on_me: kom,
                inv,
            };
            let threat = Threat {
                champ: p.champ,
                name: c.name.clone(),
                score,
                kills: p.kills,
                deaths: p.deaths,
                assists: p.assists,
                level_diff: (p.level as f32 - avg_lvl).round() as i32,
                item_gold_diff: (inv.gold as f32 - avg_gold).round() as i32,
                kills_on_me: kom,
                lane_opp: is_opp,
                damage: foe.damage_label().into(),
                flags: foe.flags(),
            };
            (foe, threat)
        })
        .collect();
    rows.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
    let (foes, threats) = rows.into_iter().unzip();
    Assessed { foes, threats, lane_opp }
}
