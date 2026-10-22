//! Lane matchups from lolalytics counters (Rift only).

use crate::consts::*;
use model::*;

pub fn matchups(snap: &GameSnapshot, stats: Option<&ChampStats>, lane_opp: Option<usize>) -> Vec<MatchupTip> {
    let (Some(stats), Some(me)) = (stats, snap.players.get(snap.me)) else { return vec![] };
    if snap.mode != Mode::Rift {
        return vec![];
    }
    let mut out: Vec<MatchupTip> = snap
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.team != me.team)
        .filter_map(|(i, p)| {
            let m = stats.counters.iter().find(|m| m.champ == p.champ)?;
            let label = match m.vs_wr {
                w if w > FAVOURED => "favoured",
                w if w < UNFAVOURED => "unfavoured",
                _ => "even",
            };
            Some(MatchupTip {
                champ: p.champ,
                vs_wr: m.vs_wr,
                games: m.games,
                lane_opp: lane_opp == Some(i),
                label: label.into(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.lane_opp.cmp(&a.lane_opp).then(a.vs_wr.total_cmp(&b.vs_wr)));
    out
}
