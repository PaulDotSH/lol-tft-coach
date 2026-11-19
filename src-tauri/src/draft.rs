//! Draft data for champ select: Rift per-lane matchups and teammate synergy for
//! revealed champions, the ARAM tier list for the bench; fetched in the background.

use crate::state::{Shared, ensure};
use model::*;
use std::sync::{Arc, OnceLock};

fn lane_stats(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Option<Lane>) -> Option<Arc<ChampStats>> {
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.lane_stats,
        |s| &s.lola_ok,
        (champ, lane),
        move || {
            let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
            async move { data::lolalytics_team::lane_stats(&http, &c, lane, &s.tier, &s.region, &patch).await }
        },
    )
}

fn team(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Lane) -> Option<Arc<TeamSynergy>> {
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.team,
        |s| &s.lola_ok,
        (champ, lane),
        move || {
            let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
            async move { data::lolalytics_team::team(&http, &c, lane, &s.tier, &s.region, &patch).await }
        },
    )
}

fn counters_vs(sh: &Arc<Shared>, sd: &StaticData, champ: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<Vec<Matchup>>> {
    let (c, patch) = (sd.champs.get(&champ)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.vs_counters,
        |s| &s.lola_ok,
        (champ, lane, vs_lane),
        move || {
            let (http, s, c) = (sh.http.clone(), sh.settings(), c.clone());
            async move { data::lolalytics::counters_vs(&http, &c, lane, vs_lane, &s.tier, &s.region, &patch).await }
        },
    )
}

pub fn matchup_page(sh: &Arc<Shared>, sd: &StaticData, me: ChampKey, vs: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<MatchupBuild>> {
    let (a, b, patch) = (sd.champs.get(&me)?, sd.champs.get(&vs)?, sh.patch()?);
    ensure(
        sh,
        |s| &s.matchups,
        |s| &s.lola_ok,
        (me, vs, lane, vs_lane),
        move || {
            let (http, s, a, b) = (sh.http.clone(), sh.settings(), a.clone(), b.clone());
            async move { data::lolalytics::matchup(&http, &a, &b, lane, vs_lane, &s.tier, &s.region, &patch).await }
        },
    )
}

/// Tips Claude wrote for me against `them`: kept under `%APPDATA%\lol-coach\ai-tips`, else asked
/// once in the background through the Claude Code CLI (about half a minute). None until they are
/// there, with the setting off, outside ranked and custom games (`queue`), before both champions'
/// spells are known, or for a pairing that has a hand-written list.
pub fn ai_tips(sh: &Arc<Shared>, sd: &StaticData, queue: i64, me: ChampKey, them: ChampKey, lane: Lane, vs_lane: Lane) -> Option<Arc<Vec<PairTip>>> {
    let (a, b) = (sd.champs.get(&me)?, sd.champs.get(&them)?);
    if !sh.settings().ai_tips || !coach::ai_queue(queue) || lane_notes().pair(&a.id, &b.id).len() > 1 {
        return None;
    }
    let spells = crate::ingame::spells(sh, sd, &[me, them]);
    let (mine, theirs) = (spells.get(&me)?.clone(), spells.get(&them)?.clone());
    let patch = sh.patch()?;
    let numbers = matchup_page(sh, sd, me, them, lane, vs_lane).map_or(vec![], |b| coach::pair_tips::numbers(sd, &b));
    ensure(sh, |s| &s.ai_tips, |s| &s.ai_ok, (me, them, lane), move || {
        let (a, b) = (a.clone(), b.clone());
        let runes: Vec<String> = sd.runes.values().map(|r| r.name.clone()).collect();
        let dir = crate::settings::config_dir().join("ai-tips");
        async move {
            let p = data::ai_tips::Pairing { me: &a.name, me_id: &a.id, them: &b.name, them_id: &b.id, lane, vs_lane, patch: &patch, my_spells: &mine, their_spells: &theirs, numbers: &numbers };
            let runes: Vec<&str> = runes.iter().map(String::as_str).collect();
            data::ai_tips::tips(&dir, &p, &runes).await
        }
    })
}

/// Rift with my lane known: the revealed enemies with the lane the draft places them in
/// (`known`: the stats whose lane shares place them, as the draft report's).
pub fn placed(cs: &ChampSelect, known: &[&ChampStats]) -> Faced {
    if cs.mode != Mode::Rift || cs.my_lane.is_none() {
        return vec![];
    }
    coach::draft_lineups(cs, known).1.iter().filter_map(|&(e, l)| Some((l?, e))).collect()
}

/// Each placed enemy's win rates against my lane's champions, lane against lane, as far as fetched.
pub fn enemy_tables(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, placed: &[(Lane, ChampKey)]) -> Vec<(ChampKey, Lane, Arc<Vec<Matchup>>)> {
    let Some(lane) = cs.my_lane else { return vec![] };
    placed.iter().filter_map(|&(l, e)| Some((e, l, counters_vs(sh, sd, e, l, lane)?))).collect()
}

/// My matchup pages against the placed enemies, as far as fetched: my direct opponent's from the
/// hover on, the others' only once my pick is made (four pages per hovered champion otherwise).
pub fn my_pages(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, placed: &[(Lane, ChampKey)]) -> Vec<Arc<MatchupBuild>> {
    let (Some(me), Some(lane)) = (cs.my_pick.or(cs.my_hover), cs.my_lane) else { return vec![] };
    let faced: Faced = coach::opposing_lanes(lane).iter().filter_map(|&l| placed.iter().find(|p| p.0 == l).copied()).collect();
    let first = direct(lane, &faced);
    placed.iter().filter(|&&p| cs.my_pick == Some(me) || Some(p) == first).filter_map(|&(l, e)| matchup_page(sh, sd, me, e, lane, l)).collect()
}

/// Hand-written laning notes, parsed once (empty if the file is broken).
pub fn lane_notes() -> &'static LaneNotes {
    static NOTES: OnceLock<LaneNotes> = OnceLock::new();
    NOTES.get_or_init(|| data::notes::lane_notes().unwrap_or_else(|e| {
        tracing::warn!("lane notes: {e:#}");
        LaneNotes::default()
    }))
}

/// Revealed enemies in the lanes I face.
type Faced = Vec<(Lane, ChampKey)>;

/// Rift: me, my lane and the revealed enemies in the opposing lane(s).
fn lane_up(cs: &ChampSelect, advice: &ChampSelectAdvice) -> Option<(ChampKey, Lane, Faced)> {
    let me = cs.my_pick.or(cs.my_hover).filter(|_| cs.mode == Mode::Rift)?;
    let lanes = advice.draft.as_ref().map_or(&[][..], |d| d.lanes.as_slice());
    let lane = cs.my_lane.or_else(|| lanes.iter().find(|l| l.ally == Some(me)).map(|l| l.lane))?;
    let enemy = |l: Lane| lanes.iter().find(|x| x.lane == l).and_then(|x| x.enemy);
    Some((me, lane, coach::opposing_lanes(lane).iter().filter_map(|&l| Some((l, enemy(l)?))).collect()))
}

/// Direct opponent: same lane, else (support with their support unknown) their bot laner.
fn direct(lane: Lane, faced: &[(Lane, ChampKey)]) -> Option<(Lane, ChampKey)> {
    faced.iter().find(|f| f.0 == lane).or(faced.first()).copied()
}

fn coachless_runes(sh: &Arc<Shared>, me: ChampKey, lane: Lane, vs: ChampKey, base_wr: f32) -> Option<Arc<Option<RuneData>>> {
    ensure(sh, |s| &s.coachless_runes, |s| &s.coachless_ok, (me, lane, vs), move || {
        let sh = sh.clone();
        async move {
            let s = sh.settings();
            let q = data::coachless::Query { champ: me, lane, vs: Some(vs), tier: &s.tier, region: &s.region, first: None, second: None };
            data::coachless::matchup_runes(&sh.http, &sh.coachless_auth, q, base_wr).await
        }
    })
}

/// My rune pages against my direct lane opponent, as fetched so far.
pub struct VsPages {
    pub vs: ChampKey,
    pub coachless: Option<Arc<Option<RuneData>>>,
    pub lolalytics: Option<Arc<MatchupBuild>>,
}

/// `base_wr`: my champion's win rate, which Coachless' WPA is added to. Coachless is asked
/// only against my own lane's opponent (signed in).
pub fn vs_runes(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice, base_wr: f32) -> Option<VsPages> {
    let (me, lane, faced) = lane_up(cs, advice)?;
    let (l, e) = direct(lane, &faced)?;
    let coachless = if l == lane && sh.coachless_auth.signed_in() { coachless_runes(sh, me, lane, e, base_wr) } else { None };
    Some(VsPages { vs: e, coachless, lolalytics: matchup_page(sh, sd, me, e, lane, l) })
}

/// Matchup card for my lane opponent(s) once my lane and champion are known;
/// lolalytics and Data Dragon parts join once fetched.
pub fn matchup(sh: &Arc<Shared>, sd: &StaticData, cs: &ChampSelect, advice: &ChampSelectAdvice) -> Option<MatchupPlan> {
    let (me, lane, faced) = lane_up(cs, advice)?;
    let draft = advice.draft.as_ref();
    let lanes = draft.map_or(&[][..], |d| d.lanes.as_slice());
    let vs: Vec<(Lane, Vec<Matchup>)> = faced.iter().filter_map(|&(l, _)| Some((l, (*counters_vs(sh, sd, me, lane, l)?).clone()))).collect();
    let build = direct(lane, &faced).and_then(|(l, e)| matchup_page(sh, sd, me, e, lane, l));
    let champs: Vec<ChampKey> = faced.iter().map(|f| f.1).collect();
    let spells = crate::ingame::spells(sh, sd, &champs);
    let mut plan = coach::matchup_plan(&coach::MatchupInput {
        cs,
        card: coach::CardData { sd, notes: lane_notes(), me, keystone: None, spells: &spells, memory: &advice.memory },
        lane,
        lanes,
        duos: draft.map_or(&[][..], |d| d.allies.duos.as_slice()),
        vs: &vs,
        build: build.as_deref(),
    })?;
    // Asked once my pick is made, not on every hover.
    if cs.my_pick == Some(me) {
        for &(l, e) in &faced {
            if let Some(tips) = ai_tips(sh, sd, cs.queue_id, me, e, lane, l) {
                coach::matchup_ai(&mut plan.cards, sd, e, &tips, None);
            }
        }
    }
    Some(plan)
}

/// ARAM win rates and tiers for the ARAM / Mayhem bench.
pub fn aram_tiers(sh: &Arc<Shared>) -> Option<Arc<Vec<TierEntry>>> {
    let patch = sh.patch()?;
    ensure(
        sh,
        |s| &s.aram_tiers,
        |s| &s.lola_ok,
        (),
        move || {
            let (http, s) = (sh.http.clone(), sh.settings());
            async move { data::lolalytics_aram::aram_tier_list(&http, &s.tier, &s.region, &patch).await }
        },
    )
}

/// Lane stats and synergy for `coach::DraftData`. `known` = stats already at
/// hand (mine, enemies in my lane); they give lane shares for placing enemies.
/// Only revealed champions are fetched: two requests each, cached for 12 h.
pub fn fetch(
    sh: &Arc<Shared>,
    sd: &StaticData,
    cs: &ChampSelect,
    known: &[&ChampStats],
) -> (Vec<ChampStats>, Vec<TeamSynergy>) {
    if cs.mode != Mode::Rift {
        return (vec![], vec![]);
    }
    let me = cs.my_pick.or(cs.my_hover);
    // Allies without an assigned lane (blind pick): their default lane's stats give lane shares.
    let mut stats: Vec<ChampStats> = cs
        .allies
        .iter()
        .filter(|a| !cs.ally_lanes.iter().any(|x| x.0 == **a))
        .filter_map(|&a| lane_stats(sh, sd, a, None))
        .map(|s| (*s).clone())
        .collect();
    let (al, en) = {
        let all: Vec<&ChampStats> = known.iter().copied().chain(&stats).collect();
        coach::draft_lineups(cs, &all)
    };
    let mut synergy = Vec::new();
    for &(c, lane) in al.iter().chain(&en) {
        // My lane's stats are `mine`; my pairs come from the allies' synergy rows.
        let Some(lane) = lane.filter(|_| Some(c) != me) else { continue };
        if !stats.iter().any(|s| s.champ == c && s.lane == Some(lane))
            && let Some(s) = lane_stats(sh, sd, c, Some(lane))
        {
            stats.push((*s).clone());
        }
        if let Some(t) = team(sh, sd, c, lane) {
            synergy.push((*t).clone());
        }
    }
    (stats, synergy)
}
