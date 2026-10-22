// Self-coaching samples: goals, death review for the mock recap (Syndra mid, 6/7/9 loss), trends.
import type { DeathReview } from '../lib/bindings/DeathReview';
import type { GameSummary } from '../lib/bindings/GameSummary';
import type { Goal } from '../lib/bindings/Goal';
import type { GoalProgress } from '../lib/bindings/GoalProgress';
import type { Trends } from '../lib/bindings/Trends';

export const goals: Goal[] = [
  { kind: 'earlydeaths', target: 1 },
  { kind: 'unspent', target: 1000 },
];

const ZED = 238;
const VI = 254;
const KAISA = 145;
const NAUTILUS = 111;
const DARIUS = 122;
const ZHONYAS = 3157;
const SHADOWFLAME = 4645;
const RABADONS = 3089;

export const review: DeathReview = {
  notes: [
    'Died 3× within 60 s of Zed reaching level 6/11',
    'Died 3× with 1000+ g unspent',
    '4 of 7 deaths were to the enemy on top of the threat board',
    'You had 1.4k less item gold than your killer on average',
  ],
  deaths: [
    { time: 371, killer: ZED, assisters: [], gold_gap: -350, killer_top_threat: false, unspent: 640, buy_next: 6655, could_buy: false, owned: [1056, 2003], spikes: [[ZED, 6]] },
    { time: 698, killer: VI, assisters: [ZED], gold_gap: 120, killer_top_threat: false, unspent: 1180, buy_next: 6655, could_buy: true, owned: [1056, 3020, 1058], spikes: [[ZED, 6]] },
    { time: 812, killer: ZED, assisters: [VI], gold_gap: -1450, killer_top_threat: true, unspent: 420, buy_next: SHADOWFLAME, could_buy: false, owned: [6655, 3020], spikes: [] },
    { time: 1003, killer: ZED, assisters: [], gold_gap: -2100, killer_top_threat: true, unspent: 1310, buy_next: ZHONYAS, could_buy: true, owned: [6655, 3020, 1058], spikes: [[ZED, 11]] },
    { time: 1290, killer: KAISA, assisters: [NAUTILUS], gold_gap: -900, killer_top_threat: false, unspent: 300, buy_next: ZHONYAS, could_buy: false, owned: [6655, 3020, SHADOWFLAME], spikes: [] },
    { time: 1544, killer: ZED, assisters: [DARIUS, NAUTILUS], gold_gap: -2600, killer_top_threat: true, unspent: 1650, buy_next: ZHONYAS, could_buy: true, owned: [6655, 3020, SHADOWFLAME, 1058], spikes: [] },
    { time: 1801, killer: ZED, assisters: [KAISA], gold_gap: -2400, killer_top_threat: true, unspent: 210, buy_next: RABADONS, could_buy: false, owned: [6655, 3020, SHADOWFLAME, 3089, 1058], spikes: [] },
  ],
};

export const summary: GameSummary = {
  ended: 1792610000,
  mode: 'rift',
  champ: 134,
  win: false,
  duration_s: 1874,
  cs_10: 62,
  deaths_before_14: 3,
  vision: 19,
  unspent_at_death: review.deaths.map((d) => d.unspent),
  buys: 4,
  followed: 2,
};

/** Final, in the recap. */
export const recapGoals: GoalProgress[] = [
  { kind: 'earlydeaths', target: 1, value: 3, met: false },
  { kind: 'unspent', target: 1000, value: 3, met: false },
];

/** Live at 18:32 in the Rift mock game. */
export const liveGoals: GoalProgress[] = [
  { kind: 'earlydeaths', target: 1, value: 1, met: true },
  { kind: 'unspent', target: 1000, value: 0, met: null },
];

export const trends: Trends = {
  trends: [
    { mode: 'rift', kind: 'cs10', series: [5.4, 5.9, 5.1, 6.2, 5.8, 6.4, 6.1, 6.8, 6.6, 7.1, 6.9], recent: 6.8, before: 5.8, better: true },
    { mode: 'rift', kind: 'earlydeaths', series: [2, 1, 3, 2, 2, 1, 3, 2, 2, 3, 3], recent: 2.4, before: 1.8, better: false },
    { mode: 'rift', kind: 'vision', series: [0.62, 0.7, 0.58, 0.66, 0.71, 0.64, 0.69, 0.6, 0.67, 0.65, 0.63], recent: 0.65, before: 0.65, better: null },
    { mode: 'rift', kind: 'unspent', series: [880, 1240, 610, 930, 720, 540], recent: 760, before: 910, better: true },
    { mode: 'rift', kind: 'buynext', series: [50, 75, 60, 80, 50, 67], recent: 66, before: 62, better: true },
    { mode: 'mayhem', kind: 'unspent', series: [1500, 1900, 1250, 1100], recent: 1440, before: null, better: null },
  ],
  follow: [{ mode: 'rift', wins: 78, losses: 54, games: 9 }],
  augments: [
    { id: 1030, games: 3, damage_per_min: 2140 },
    { id: 2132, games: 2, damage_per_min: 1860 },
    { id: 1389, games: 2, damage_per_min: 2510 },
    { id: 1053, games: 1, damage_per_min: 1720 },
  ],
  goals: [
    { goal: goals[0], met: 4, of: 10 },
    { goal: goals[1], met: 3, of: 6 },
  ],
};
