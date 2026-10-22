//! Objective respawn timers from game events (Rift only).

use crate::consts::*;
use model::*;

/// Rift only. Times are game seconds; `next_spawn` None = up now.
pub fn timers(snap: &GameSnapshot) -> Vec<ObjectiveTimer> {
    if snap.mode != Mode::Rift {
        return vec![];
    }
    let now = snap.time;
    let (mut drakes, mut last_drake, mut last_elder, mut soul_at) = ([0u32; 2], None::<f32>, None::<f32>, None::<f32>);
    let (mut last_baron, mut herald_taken, mut grubs) = (None::<f32>, false, 0u32);
    for e in &snap.events {
        match e {
            GameEvent::Dragon { time, kind, .. } if kind.to_ascii_lowercase().contains("elder") => last_elder = Some(*time),
            GameEvent::Dragon { time, team, .. } => {
                last_drake = Some(*time);
                if let Some(t) = team {
                    let c = &mut drakes[*t as usize];
                    *c += 1;
                    if *c == SOUL_DRAGONS && soul_at.is_none() {
                        soul_at = Some(*time);
                    }
                }
            }
            GameEvent::Baron { time, .. } => last_baron = Some(*time),
            GameEvent::Herald { .. } => herald_taken = true,
            GameEvent::Objective { name, .. } => {
                let n = name.to_ascii_lowercase();
                if n.contains("grub") || n.contains("horde") {
                    grubs += 1;
                }
            }
            _ => {}
        }
    }
    let at = |t: f32| (t > now).then_some(t);
    let mut out = Vec::with_capacity(4);
    match soul_at {
        Some(s) => {
            let next = last_elder.map_or(s, |e| e.max(s)) + ELDER_RESPAWN;
            out.push(ObjectiveTimer { name: "Elder".into(), next_spawn: at(next) });
        }
        None => {
            let next = last_drake.map_or(DRAGON_FIRST, |t| t + DRAGON_RESPAWN);
            out.push(ObjectiveTimer { name: "Dragon".into(), next_spawn: at(next) });
        }
    }
    out.push(ObjectiveTimer {
        name: "Baron".into(),
        next_spawn: at(last_baron.map_or(BARON_FIRST, |t| t + BARON_RESPAWN)),
    });
    if !herald_taken && now < BARON_FIRST {
        out.push(ObjectiveTimer { name: "Herald".into(), next_spawn: at(HERALD_FIRST) });
    }
    if grubs < GRUBS_COUNT && now < HERALD_FIRST {
        out.push(ObjectiveTimer { name: "Void Grubs".into(), next_spawn: at(GRUBS_FIRST) });
    }
    out
}
