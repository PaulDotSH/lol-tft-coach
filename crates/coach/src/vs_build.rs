//! Item tables against particular enemies, in the shape the item engine reads (`ChampStats`
//! slots and boots): the lolalytics matchup page against my lane opponent on its own, and my
//! pages against every revealed enemy added to my champion's own tables.

use crate::consts::GAME_PAGE_GAMES;
use model::*;

/// `rows` added into `into`, scaled down to count for at most `GAME_PAGE_GAMES` games together
/// (never up: a thin page weighs what its games are worth).
fn add(into: &mut Vec<ItemStat>, rows: &[ItemStat]) {
    let total: u32 = rows.iter().map(|r| r.games).sum();
    let f = (GAME_PAGE_GAMES as f32 / total.max(1) as f32).min(1.0);
    for r in rows {
        let (games, wins) = ((r.games as f32 * f).round() as u32, (r.wins as f32 * f).round() as u32);
        if games == 0 {
            continue;
        }
        match into.iter_mut().find(|x| x.id == r.id) {
            Some(x) => (x.games, x.wins) = (x.games + games, x.wins + wins),
            None => into.push(ItemStat { id: r.id, games, wins }),
        }
    }
}

fn merged<'a>(tables: impl Iterator<Item = &'a [ItemStat]>) -> Vec<ItemStat> {
    let mut out = Vec::new();
    tables.for_each(|t| add(&mut out, t));
    out.sort_by(|a, b| b.games.cmp(&a.games).then(a.id.cmp(&b.id)));
    out
}

fn slot(s: &[Vec<ItemStat>], k: usize) -> &[ItemStat] {
    s.get(k).map_or(&[], Vec::as_slice)
}

/// Against the whole enemy team: my champion's own tables and each page's, every one of them
/// a source of equal weight once it has `GAME_PAGE_GAMES` games in a slot. No page: my own stats.
pub fn game(mine: &ChampStats, pages: &[&MatchupBuild]) -> ChampStats {
    if pages.is_empty() {
        return mine.clone();
    }
    let slots = pages.iter().map(|p| p.slots.len()).fold(mine.slots.len(), usize::max);
    ChampStats {
        slots: (0..slots).map(|k| merged(std::iter::once(slot(&mine.slots, k)).chain(pages.iter().map(|p| slot(&p.slots, k))))).collect(),
        boots: merged(std::iter::once(mine.boots.as_slice()).chain(pages.iter().map(|p| p.boots.as_slice()))),
        ..mine.clone()
    }
}

/// Against one opponent: the page's tables, starting items, win rate and games as my stats.
pub fn lane(mine: &ChampStats, page: &MatchupBuild) -> ChampStats {
    ChampStats {
        patch: if page.patch.is_empty() { mine.patch.clone() } else { page.patch.clone() },
        win_rate: page.win_rate,
        games: page.games as u64,
        slots: page.slots.clone(),
        boots: page.boots.clone(),
        start: if page.start.is_empty() { mine.start.clone() } else { page.start.clone() },
        coachless: None,
        ..mine.clone()
    }
}
