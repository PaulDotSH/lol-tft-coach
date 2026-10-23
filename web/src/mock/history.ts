// Match history samples: AppState.history and the analyze_game result.
// analysis.json is the real review of a recorded flex game (`--example review`).
import type { MatchAnalysis } from '../lib/bindings/MatchAnalysis';
import type { PastGame } from '../lib/bindings/PastGame';
import recorded from './analysis.json';

const reviewed = recorded as MatchAnalysis;
const HOUR = 3_600_000;
const t0 = Date.now() - 2 * HOUR;

function game(i: number, p: Partial<PastGame>): PastGame {
  return {
    ...reviewed.game,
    game_id: reviewed.game.game_id - i,
    created: t0 - i * 5 * HOUR,
    death_times: null,
    gold_diff_10: null,
    gold_diff_15: null,
    xp_diff_15: null,
    cs_10: null,
    ...p,
  };
}

export const history: PastGame[] = [
  { ...reviewed.game, created: t0 },
  game(1, { champ: 111, win: true, kills: 2, deaths: 4, assists: 17, duration_s: 1747 }),
  game(2, { champ: 412, win: true, kills: 1, deaths: 5, assists: 19, duration_s: 1644 }),
  game(3, { queue_id: 2400, mode: 'mayhem', champ: 4, lane: null, win: true, kills: 14, deaths: 6, assists: 21, duration_s: 912, augments: [1077, 1041] }),
  game(4, { queue_id: 420, champ: 103, lane: 'middle', win: false, kills: 4, deaths: 7, assists: 6, cs: 202, duration_s: 1874 }),
  game(5, { queue_id: 450, mode: 'aram', champ: 81, lane: null, win: false, kills: 9, deaths: 11, assists: 17, duration_s: 1210 }),
  game(6, { queue_id: 420, champ: 134, lane: 'middle', win: true, kills: 8, deaths: 2, assists: 7, cs: 241, duration_s: 1530 }),
  game(7, { champ: 111, win: false, remake: true, kills: 0, deaths: 1, assists: 0, duration_s: 214 }),
];

/** The recorded review, wearing the clicked game's header when it is another one. */
export function analysisFor(id: number): MatchAnalysis {
  const g = history.find((h) => h.game_id === id);
  return g && id !== reviewed.game.game_id ? { ...reviewed, game: g, live: false } : reviewed;
}
