//! Lane matchup tips: my lane opponent(s) with the hand-written notes, their spell
//! cooldowns, my win rate and my record against them. Champ select plan (with my
//! bot lane partner and the lolalytics matchup page) and the in-game laning panel.

use model::*;
use std::collections::HashMap;

/// Matchup pages from fewer games than this aren't worth showing.
const MIN_MATCHUP_GAMES: u32 = 100;
const KEYS: [&str; 4] = ["Q", "W", "E", "R"];

/// What every card draws on besides the opponent.
#[derive(Clone, Copy)]
pub struct CardData<'a> {
    pub sd: &'a StaticData,
    pub notes: &'a LaneNotes,
    /// My champion (pair notes).
    pub me: ChampKey,
    /// Data Dragon Q W E R per champion, as fetched so far.
    pub spells: &'a HashMap<ChampKey, Vec<SpellInfo>>,
    /// My record against each enemy (`matchup_memory`).
    pub memory: &'a [MatchupMemory],
}

fn tags(n: &ChampNote) -> Vec<String> {
    [(n.level2, "strong level 2"), (n.level6, "all-in at 6"), (n.engage, "hard engage"), (n.poke, "poke"), (n.sustain, "sustain")]
        .into_iter()
        .filter(|t| t.0)
        .map(|t| t.1.to_string())
        .collect()
}

/// `vs`: my win rate against them and its games. `level`: theirs in game (R cooldown now).
pub fn card(d: &CardData, champ: ChampKey, vs: Option<(f32, u32)>, level: Option<u32>) -> MatchupCard {
    let id = |c: ChampKey| d.sd.champs.get(&c).map(|c| c.id.as_str());
    let note = id(champ).and_then(|i| d.notes.champs.get(i));
    let watch = note.map_or("", |n| n.watch.as_str());
    let spells = d.spells.get(&champ).map_or(vec![], |s| {
        s.iter()
            .zip(KEYS)
            .map(|(s, k)| {
                let r = UltCooldowns { max_rank: s.cooldown.len() as u32, cooldown: s.cooldown.clone() };
                let now = level.filter(|_| k == "R").and_then(|l| crate::spikes::ult(l, Some(&r))).and_then(|u| u.cooldowns.get(u.rank as usize - 1).copied());
                SpellCd { key: k.into(), name: s.name.clone(), icon: s.icon.clone(), cooldown: s.cooldown.clone(), watch: watch.contains(k), now }
            })
            .collect()
    });
    MatchupCard {
        champ,
        vs_wr: vs.map(|v| v.0),
        games: vs.map_or(0, |v| v.1),
        tags: note.map_or(vec![], tags),
        note: note.cloned(),
        pair: id(d.me).zip(id(champ)).and_then(|(m, t)| d.notes.pair(m, t)).cloned(),
        spells,
        record: d.memory.iter().find(|m| m.champ == champ).cloned(),
    }
}

/// Lanes whose enemies I face: both bot lanes for a support or ADC.
pub fn opposing(lane: Lane) -> &'static [Lane] {
    match lane {
        Lane::Bottom | Lane::Support => &[Lane::Bottom, Lane::Support],
        Lane::Top => &[Lane::Top],
        Lane::Jungle => &[Lane::Jungle],
        Lane::Middle => &[Lane::Middle],
    }
}

pub struct PlanInput<'a> {
    pub cs: &'a ChampSelect,
    pub card: CardData<'a>,
    /// Assigned lane, else my champion's lolalytics lane.
    pub lane: Lane,
    /// Draft lanes (enemies placed by lane shares).
    pub lanes: &'a [DraftLane],
    /// My team's duo synergy.
    pub duos: &'a [DraftDuo],
    /// My win rates against each opposing lane (lolalytics `vslane`), by their lane.
    pub vs: &'a [(Lane, Vec<Matchup>)],
    /// Matchup page against my direct opponent (their bot laner when their support is unknown).
    pub build: Option<&'a MatchupBuild>,
}

pub fn plan(i: &PlanInput) -> Option<MatchupPlan> {
    let (cs, lane) = (i.cs, i.lane);
    if cs.mode != Mode::Rift {
        return None;
    }
    let me = cs.my_pick.or(cs.my_hover)?;
    let at = |l: Lane| i.lanes.iter().find(|x| x.lane == l);
    let other = match lane {
        Lane::Bottom => Some(Lane::Support),
        Lane::Support => Some(Lane::Bottom),
        _ => None,
    };
    let partner = other.and_then(|o| at(o).and_then(|x| x.ally).or_else(|| cs.ally_lanes.iter().find(|x| x.1 == o).map(|x| x.0)));
    let cards: Vec<MatchupCard> = opposing(lane)
        .iter()
        .filter_map(|&l| {
            let e = at(l)?.enemy?;
            let vs = i.vs.iter().find(|v| v.0 == l).and_then(|v| v.1.iter().find(|m| m.champ == e)).map(|m| (m.vs_wr, m.games));
            Some(card(&i.card, e, vs, None))
        })
        .collect();
    let build = i.build.filter(|m| m.games >= MIN_MATCHUP_GAMES && cards.iter().any(|c| c.champ == m.vs)).cloned();
    let runes = build.as_ref().map_or(vec![], |b| {
        b.runes
            .iter()
            .filter_map(|id| {
                let r = i.card.sd.runes.get(id)?;
                Some(RunePick { id: *id, name: r.name.clone(), icon: r.icon.clone(), reason: None })
            })
            .collect()
    });
    Some(MatchupPlan {
        lane,
        allies: std::iter::once(me).chain(partner).collect(),
        partner: partner.and_then(|p| {
            let d = i.duos.iter().find(|d| (d.a, d.b) == (me, p) || (d.a, d.b) == (p, me))?;
            Some(PartnerSynergy { champ: p, delta: d.delta, games: d.games })
        }),
        cards,
        build,
        runes,
    })
}

/// In-game laning panel: my lane opponent and, in bot lane, the other enemy bot
/// laner (bot laner first). Win rates come from `advice.matchups`.
pub fn laning(advice: &GameAdvice, snap: &GameSnapshot, d: &CardData) -> Vec<MatchupCard> {
    let Some(me) = snap.players.get(snap.me) else { return vec![] };
    if snap.mode != Mode::Rift || crate::jungle::is_jungler(me) {
        return vec![];
    }
    let enemies = || snap.players.iter().filter(|p| p.team != me.team);
    let Some(opp) = advice.threats.iter().find(|t| t.lane_opp).and_then(|t| enemies().find(|p| p.champ == t.champ)) else { return vec![] };
    let bot = |l: Option<Lane>| matches!(l, Some(Lane::Bottom | Lane::Support));
    let mut faced = vec![opp];
    if bot(me.position) || bot(opp.position) {
        faced.extend(enemies().filter(|p| p.champ != opp.champ && bot(p.position)).take(1));
        faced.sort_by_key(|p| p.position == Some(Lane::Support));
    }
    faced
        .iter()
        .map(|p| {
            let vs = advice.matchups.iter().find(|m| m.champ == p.champ).map(|m| (m.vs_wr, m.games));
            card(d, p.champ, vs, Some(p.level))
        })
        .collect()
}

