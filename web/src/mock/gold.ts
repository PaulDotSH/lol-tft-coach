// Item-gold diff samples for the in-game mock fixtures, consistent with their threats and self stats.
import type { GoldDiff } from '../lib/bindings/GoldDiff';
import type { GoldSample } from '../lib/bindings/GoldSample';

/** One sample every 20 s up to `end`, ending exactly on the current values. */
function history(end: number, team: (t: number) => number, lane: ((t: number) => number) | null): GoldSample[] {
  const out: GoldSample[] = [];
  for (let t = 0; t < end; t += 20) out.push({ time: t, team: Math.round(team(t)), lane: lane && Math.round(lane(t)), win: null });
  return out;
}

/** The same samples with `win` (`WinProb::base`) from the team item-gold diff, `perK` per 1k, and a
 *  drift over the game standing in for scaling and objectives. */
const withWin = (h: GoldSample[], end: number, perK: number, drift: number): GoldSample[] =>
  h.map((s) => ({ ...s, win: Math.round(1000 / (1 + Math.exp(-((perK * s.team) / 1000 + (drift * s.time) / end)))) / 1000 }));

const RIFT_END = 1112;
const riftHistory = history(
  RIFT_END,
  (t) => 1300 * Math.sin(t / 240) * Math.exp(-t / 700) - 2700 * (t / RIFT_END) ** 2.2,
  (t) => 250 * Math.sin(t / 110) - 2100 * (t / RIFT_END) ** 1.6,
);
riftHistory.push({ time: RIFT_END, team: -2700, lane: -2100, win: null });

// Syndra mid (me): Ornn, Sejuani, Jinx, Lulu vs Darius, Vi, Zed, Kai'Sa, Nautilus.
export const riftGold: GoldDiff = {
  team_item_gold: [24700, 27400],
  diff: -2700,
  kills: [14, 19],
  level_diff: -0.4,
  lanes: [
    { lane: 'top', ally: 516, enemy: 122, gold_diff: 600, level_diff: 1, mine: false },
    { lane: 'jungle', ally: 113, enemy: 254, gold_diff: -300, level_diff: 0, mine: false },
    { lane: 'middle', ally: 134, enemy: 238, gold_diff: -2100, level_diff: -2, mine: true },
    { lane: 'bottom', ally: 222, enemy: 145, gold_diff: -1200, level_diff: -1, mine: false },
    { lane: 'support', ally: 117, enemy: 111, gold_diff: 300, level_diff: 0, mine: false },
  ],
  players: [
    { champ: 222, ally: true, item_gold: 6300, level: 11, mine: false },
    { champ: 516, ally: true, item_gold: 5800, level: 13, mine: false },
    { champ: 134, ally: true, item_gold: 5150, level: 11, mine: true },
    { champ: 113, ally: true, item_gold: 4900, level: 11, mine: false },
    { champ: 117, ally: true, item_gold: 2550, level: 9, mine: false },
    { champ: 145, ally: false, item_gold: 7500, level: 12, mine: false },
    { champ: 238, ally: false, item_gold: 7250, level: 13, mine: false },
    { champ: 122, ally: false, item_gold: 5200, level: 12, mine: false },
    { champ: 254, ally: false, item_gold: 5200, level: 11, mine: false },
    { champ: 111, ally: false, item_gold: 2250, level: 9, mine: false },
  ],
  history: riftHistory,
};

// Same game, scored: behind on gold and dragons, the draft scaling better (win chance 33 %).
export const riftGoldWin: GoldDiff = { ...riftGold, history: withWin(riftHistory, RIFT_END, 0.33, 0.2) };

const AHEAD_END = 1340;
const aheadHistory = history(
  AHEAD_END,
  (t) => 3100 * (t / AHEAD_END) ** 1.8 - 900 * Math.sin(t / 300) * (1 - t / AHEAD_END),
  (t) => 200 * Math.sin(t / 120) - 1600 * (t / AHEAD_END) ** 1.4,
);
aheadHistory.push({ time: AHEAD_END, team: 3100, lane: -1600, win: null });

// The same ten champions at 22:20 with my team ahead (Jinx fed bot, Zed still ahead of me mid).
export const riftAheadGold: GoldDiff = {
  team_item_gold: [30900, 27800],
  diff: 3100,
  kills: [17, 11],
  level_diff: 0.8,
  lanes: [
    { lane: 'top', ally: 516, enemy: 122, gold_diff: 900, level_diff: 1, mine: false },
    { lane: 'jungle', ally: 113, enemy: 254, gold_diff: 700, level_diff: 1, mine: false },
    { lane: 'middle', ally: 134, enemy: 238, gold_diff: -1600, level_diff: -1, mine: true },
    { lane: 'bottom', ally: 222, enemy: 145, gold_diff: 2300, level_diff: 2, mine: false },
    { lane: 'support', ally: 117, enemy: 111, gold_diff: 800, level_diff: 1, mine: false },
  ],
  players: [
    { champ: 222, ally: true, item_gold: 8100, level: 15, mine: false },
    { champ: 516, ally: true, item_gold: 6600, level: 15, mine: false },
    { champ: 113, ally: true, item_gold: 6150, level: 13, mine: false },
    { champ: 134, ally: true, item_gold: 6050, level: 13, mine: true },
    { champ: 117, ally: true, item_gold: 4000, level: 12, mine: false },
    { champ: 238, ally: false, item_gold: 7650, level: 14, mine: false },
    { champ: 145, ally: false, item_gold: 5800, level: 13, mine: false },
    { champ: 122, ally: false, item_gold: 5700, level: 14, mine: false },
    { champ: 254, ally: false, item_gold: 5450, level: 12, mine: false },
    { champ: 111, ally: false, item_gold: 3200, level: 11, mine: false },
  ],
  history: withWin(aheadHistory, AHEAD_END, 0.2, 0.09),
};

const MAYHEM_END = 412;
const mayhemHistory = history(MAYHEM_END, (t) => 1250 * (t / MAYHEM_END) + 700 * Math.sin(t / 60) * (1 - t / MAYHEM_END), null);
mayhemHistory.push({ time: MAYHEM_END, team: 1250, lane: null, win: null });

// Ahri (me), Garen, Lux, Ezreal, Jinx vs Katarina, Master Yi, Dr. Mundo, Soraka, Ashe.
export const mayhemGold: GoldDiff = {
  team_item_gold: [19850, 18600],
  diff: 1250,
  kills: [21, 18],
  level_diff: 0.2,
  lanes: [],
  players: [
    { champ: 103, ally: true, item_gold: 4650, level: 9, mine: true },
    { champ: 81, ally: true, item_gold: 4300, level: 9, mine: false },
    { champ: 222, ally: true, item_gold: 4100, level: 9, mine: false },
    { champ: 86, ally: true, item_gold: 3700, level: 8, mine: false },
    { champ: 99, ally: true, item_gold: 3100, level: 8, mine: false },
    { champ: 55, ally: false, item_gold: 4900, level: 9, mine: false },
    { champ: 11, ally: false, item_gold: 4200, level: 9, mine: false },
    { champ: 36, ally: false, item_gold: 3600, level: 8, mine: false },
    { champ: 22, ally: false, item_gold: 3300, level: 8, mine: false },
    { champ: 16, ally: false, item_gold: 2600, level: 8, mine: false },
  ],
  history: mayhemHistory,
};

// Same Mayhem game, scored by the ARAM model: ahead on items, their draft scaling better (58 %).
export const mayhemGoldWin: GoldDiff = { ...mayhemGold, history: withWin(mayhemHistory, MAYHEM_END, 0.38, -0.15) };
