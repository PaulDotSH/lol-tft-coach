//! Post-game recap: per-minute numbers vs benchmarks.

use crate::consts::*;
use model::*;

fn k(n: f32) -> String {
    if n >= 1000.0 { format!("{:.1}k", n / 1000.0) } else { format!("{n:.0}") }
}

pub fn recap(eog: &EogStats, b: &Benchmarks, recommended: &[ItemId], sd: &StaticData) -> Recap {
    let min = (eog.duration_s as f32 / 60.0).max(1.0);
    let pm = |v: u32| v as f32 / min;
    let mut notes = vec![format!("{}/{}/{} on {}", eog.kills, eog.deaths, eog.assists, sd.champ_name(eog.champ))];
    notes.push(match b.cs_per_min {
        Some(x) => format!("CS {:.1}/min vs {x:.1} avg", pm(eog.cs)),
        None => format!("CS {:.1}/min", pm(eog.cs)),
    });
    if let Some(x) = b.gold_per_min {
        notes.push(format!("Gold {:.0}/min vs {x:.0} avg", pm(eog.gold)));
    }
    notes.push(match b.damage_per_min {
        Some(x) => format!("Damage {:.0}/min vs {x:.0} avg", pm(eog.damage_to_champs)),
        None => format!("Dealt {} damage to champions", k(eog.damage_to_champs as f32)),
    });
    notes.push(format!("Took {} damage ({:.0}/min)", k(eog.damage_taken as f32), pm(eog.damage_taken)));
    if eog.mode == Mode::Rift {
        notes.push(match b.vision_per_min {
            Some(x) => format!("Vision {:.1}/min vs {x:.1} avg", pm(eog.vision)),
            None => format!("Vision score {}", eog.vision),
        });
    }
    if eog.deaths >= DEATHS_NOTE {
        notes.push(format!("Died {}× ({:.1}/10 min)", eog.deaths, eog.deaths as f32 / min * 10.0));
    }
    if !recommended.is_empty() {
        let n = recommended.iter().filter(|r| eog.items.contains(r)).count();
        notes.push(format!("Built {n} of {} recommended items", recommended.len()));
    }
    Recap { stats: eog.clone(), benchmarks: b.clone(), recommended: recommended.to_vec(), notes }
}
