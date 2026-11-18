// Match history samples: AppState.history and the analyze_game result.
// analysis.json is the real review of a recorded flex game (`--example review`); its win curve
// and swings come from the hand-set table (`fixtures/coach/winprob_handset.toml`), not a fit.
import type { BuildReview } from '../lib/bindings/BuildReview';
import type { JungleReview } from '../lib/bindings/JungleReview';
import type { MatchAnalysis } from '../lib/bindings/MatchAnalysis';
import type { PastGame } from '../lib/bindings/PastGame';
import recorded from './analysis.json';

// Made-up numbers in the shape of the crawl's build table for Nautilus support.
const build: BuildReview = {
  patch: '16.22',
  games: 1412,
  top_games: 1188,
  items: [
    { slot: 'first item', item: 3190, time: 812, all: [655, 840], top: [640, 815] },
    { slot: 'second item', item: 2504, time: 1310, all: [1020, 1230], top: [1000, 1205] },
    { slot: 'third item', item: 3076, time: 1640, all: [1390, 1650], top: [1370, 1620] },
    { slot: 'boots', item: 3047, time: 905, all: [560, 820], top: [540, 800] },
  ],
  back: { time: 290, gold: 980, all: [210, 300], all_gold: [420, 900], top: [205, 290], usual: 1028 },
  tips: ['Second item at 21:50; Emerald+ finish it between 17:00 and 20:30.'],
};
const reviewed = { ...(recorded as unknown as MatchAnalysis), build };
const HOUR = 3_600_000;
const t0 = Date.now() - 2 * HOUR;

// Deterministic death spots scattered around a few hot zones.
function spots(i: number, n: number): PastGame['death_spots'] {
  const hot = [
    [9800, 6200],
    [7400, 7400],
    [12900, 3400],
    [5200, 9600],
  ];
  return Array.from({ length: n }, (_, k) => {
    const [x, y] = hot[(i + k) % hot.length];
    return { time: 200 + k * 290 + i * 13, x: x + ((i * 733 + k * 911) % 1800) - 900, y: y + ((i * 397 + k * 577) % 1800) - 900 };
  });
}

function game(i: number, p: Partial<PastGame>): PastGame {
  return {
    ...reviewed.game,
    game_id: reviewed.game.game_id - i,
    created: t0 - i * 5 * HOUR,
    gold_diff_10: null,
    gold_diff_15: null,
    xp_diff_15: null,
    cs_10: null,
    death_times: [],
    death_spots: p.mode && p.mode !== 'rift' ? [] : spots(i, p.deaths ?? 3),
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
  game(8, { champ: 131, lane: 'jungle', win: true, kills: 7, deaths: 3, assists: 9, cs: 182, duration_s: 1786 }),
];

// The recorded flex game reviewed as its blue-side jungler (`fixtures/clients/match_game_jungle.json`).
const jungle: JungleReview = {
  enemy: 234,
  first_clear: [
    { minute: 3, level: 4, jungle_cs: 20 },
    { minute: 4, level: 4, jungle_cs: 24 },
  ],
  clear: [
    { from: 0, to: 5, jungle_per_min: 5.6, cs_per_min: 5.6 },
    { from: 5, to: 10, jungle_per_min: 7.6, cs_per_min: 7.8 },
    { from: 10, to: 15, jungle_per_min: 9.6, cs_per_min: 10.6 },
  ],
  benchmark: 6.4,
  ganks: [
    { lane: 'middle', ganks: 1, kills: 1, deaths: 0 },
    { lane: 'bottom', ganks: 2, kills: 2, deaths: 1 },
  ],
  objectives: [
    { time: 503, kind: 'Grubs', detail: '×3', ours: true, presence: 'there', enemy_jungler_dead: false },
    { time: 526, kind: 'Dragon', detail: 'Hextech', ours: false, presence: 'there', enemy_jungler_dead: false },
    { time: 932, kind: 'Dragon', detail: 'Infernal', ours: true, presence: 'there', enemy_jungler_dead: true },
    { time: 1289, kind: 'Dragon', detail: 'Ocean', ours: true, presence: 'there', enemy_jungler_dead: false },
    { time: 1693, kind: 'Baron', detail: null, ours: true, presence: 'there', enemy_jungler_dead: false },
  ],
  invades: [3, 4, 5, 8, 9, 14],
  versus: [
    { minute: 5, cs: 28, level: 4, opp_cs: 24, opp_level: 4, gold_lead: 617 },
    { minute: 10, cs: 66, level: 7, opp_cs: 56, opp_level: 7, gold_lead: 931 },
    { minute: 15, cs: 114, level: 10, opp_cs: 76, opp_level: 9, gold_lead: 1785 },
  ],
};

/** The recorded review, wearing the clicked game's header when it is another one. */
export function analysisFor(id: number): MatchAnalysis {
  const g = history.find((h) => h.game_id === id);
  if (g?.lane === 'jungle') return { ...reviewed, game: g, live: false, laning: null, jungle };
  return g && id !== reviewed.game.game_id ? { ...reviewed, game: g, live: false } : reviewed;
}
