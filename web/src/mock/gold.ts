// Item-gold diff samples for the in-game mock fixtures, consistent with their threats and self stats.
import type { GoldDiff } from '../lib/bindings/GoldDiff';
import type { GoldSample } from '../lib/bindings/GoldSample';

/** One sample every 20 s up to `end`, ending exactly on the current values. */
function history(end: number, team: (t: number) => number, lane: ((t: number) => number) | null): GoldSample[] {
  const out: GoldSample[] = [];
  for (let t = 0; t < end; t += 20) out.push({ time: t, team: Math.round(team(t)), lane: lane && Math.round(lane(t)) });
  return out;
}

const RIFT_END = 1112;
const riftHistory = history(
  RIFT_END,
  (t) => 1300 * Math.sin(t / 240) * Math.exp(-t / 700) - 2700 * (t / RIFT_END) ** 2.2,
  (t) => 250 * Math.sin(t / 110) - 2100 * (t / RIFT_END) ** 1.6,
);
riftHistory.push({ time: RIFT_END, team: -2700, lane: -2100 });

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

const MAYHEM_END = 412;
const mayhemHistory = history(MAYHEM_END, (t) => 1250 * (t / MAYHEM_END) + 700 * Math.sin(t / 60) * (1 - t / MAYHEM_END), null);
mayhemHistory.push({ time: MAYHEM_END, team: 1250, lane: null });

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
