//! Matchup memory (champ select, laning panel): my record against each enemy,
//! from the match-history cache. Facts only, same mode, remakes left out.

use model::*;

/// Kills on me before this game time count as early (laning).
const EARLY_S: u32 = 15 * 60;
/// Newest results kept per enemy.
const LAST: usize = 3;

/// `games`: my recent games, any order; `me`: the champion I play now. One entry
/// per enemy I have played against.
pub fn matchup_memory(games: &[PastGame], enemies: &[ChampKey], mode: Mode, me: Option<ChampKey>) -> Vec<MatchupMemory> {
    enemies
        .iter()
        .filter_map(|&champ| {
            let mut vs: Vec<&PastGame> = games.iter().filter(|g| g.mode == mode && !g.remake && g.enemies.contains(&champ)).collect();
            if vs.is_empty() {
                return None;
            }
            vs.sort_by_key(|g| std::cmp::Reverse(g.created));
            let lane: Vec<&PastGame> = vs.iter().copied().filter(|g| g.opponent == Some(champ)).collect();
            let diffs: Vec<i32> = lane.iter().filter_map(|g| g.gold_diff_15).collect();
            let same: Vec<&PastGame> = vs.iter().copied().filter(|g| Some(g.champ) == me).collect();
            let early = |g: &PastGame| {
                let times = g.death_times.as_deref().unwrap_or_default();
                g.killed_by.iter().zip(times).filter(|(k, t)| **k == Some(champ) && **t < EARLY_S).count() as u32
            };
            Some(MatchupMemory {
                champ,
                games: vs.len() as u32,
                wins: vs.iter().filter(|g| g.win).count() as u32,
                lane_games: lane.len() as u32,
                gold_15: (!diffs.is_empty()).then(|| diffs.iter().sum::<i32>() / diffs.len() as i32),
                killed_me: vs.iter().map(|g| g.killed_by.iter().filter(|k| **k == Some(champ)).count() as u32).sum(),
                early_kills: vs.iter().map(|g| early(g)).sum(),
                same_games: same.len() as u32,
                same_wins: same.iter().filter(|g| g.win).count() as u32,
                last: vs.iter().take(LAST).map(|g| g.win).collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(win: bool, enemies: &[ChampKey], opponent: Option<ChampKey>, gold_15: Option<i32>, killed_by: &[ChampKey]) -> PastGame {
        PastGame {
            game_id: 1,
            created: 0,
            queue_id: 420,
            mode: Mode::Rift,
            champ: 103,
            lane: Some(Lane::Middle),
            win,
            remake: false,
            duration_s: 1800,
            kills: 0,
            deaths: killed_by.len() as u32,
            assists: 0,
            cs: 0,
            gold: 0,
            damage_to_champs: 0,
            vision: 0,
            death_times: Some((0..killed_by.len() as u32).map(|i| 300 + 600 * i).collect()),
            gold_diff_10: None,
            gold_diff_15: gold_15,
            xp_diff_15: None,
            ours: None,
            theirs: None,
            first_dragon: None,
            first_tower: None,
            first_blood: None,
            cs_10: None,
            augments: vec![],
            death_spots: vec![],
            enemies: enemies.to_vec(),
            opponent,
            killed_by: killed_by.iter().map(|&k| Some(k)).collect(),
            control_wards: None,
        }
    }

    #[test]
    fn record_lane_gold_and_kills_per_enemy() {
        const ZED: ChampKey = 238;
        const LUX: ChampKey = 99;
        const AHRI: ChampKey = 103;
        let games = [
            // Deaths at 5:00, 15:00 and 25:00: only the first is early.
            PastGame { created: 3, ..game(false, &[ZED, LUX], Some(ZED), Some(-800), &[ZED, ZED, LUX]) },
            PastGame { created: 1, champ: 1, ..game(true, &[ZED], Some(ZED), Some(200), &[]) },
            PastGame { created: 2, ..game(true, &[ZED], None, None, &[ZED]) },
            PastGame { remake: true, ..game(false, &[ZED], Some(ZED), Some(-5000), &[ZED]) },
            PastGame { mode: Mode::Aram, ..game(false, &[ZED], None, None, &[ZED]) },
        ];
        let m = matchup_memory(&games, &[ZED, LUX, 1], Mode::Rift, Some(AHRI));
        let zed = MatchupMemory {
            champ: ZED,
            games: 3,
            wins: 2,
            lane_games: 2,
            gold_15: Some(-300),
            killed_me: 3,
            early_kills: 2,
            same_games: 2,
            same_wins: 1,
            last: vec![false, true, true],
        };
        let lux = MatchupMemory {
            champ: LUX,
            games: 1,
            wins: 0,
            lane_games: 0,
            gold_15: None,
            killed_me: 1,
            early_kills: 0,
            same_games: 1,
            same_wins: 0,
            last: vec![false],
        };
        assert_eq!(m, [zed, lux]);
    }
}
