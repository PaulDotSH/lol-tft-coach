//! Death windows (Q6): who is dead now and when they are back, as the
//! scoreboard shows it. Facts only.

use model::*;

pub fn deaths(snap: &GameSnapshot) -> DeathWindow {
    let Some(me) = snap.players.get(snap.me) else { return DeathWindow { enemies: vec![], allies: vec![] } };
    let side = |ally: bool| {
        let mut v: Vec<Respawn> = snap
            .players
            .iter()
            .enumerate()
            .filter(|&(i, p)| p.dead && (p.team == me.team) == ally && i != snap.me)
            .map(|(_, p)| Respawn { champ: p.champ, in_s: p.respawn_in.max(0.0) })
            .collect();
        v.sort_by(|a, b| a.in_s.total_cmp(&b.in_s));
        v
    };
    DeathWindow { enemies: side(false), allies: side(true) }
}
