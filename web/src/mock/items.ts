// Mock data for resists / EHP, detected augments and full-build gold sinks.
import type { Defense } from '../lib/bindings/Defense';
import type { GoldSink } from '../lib/bindings/GoldSink';
import type { MyAugment } from '../lib/bindings/MyAugment';

export const riftDefense: Defense = {
  ehp_physical: 3110,
  ehp_magic: 1920,
  enemy_magic: 0.18,
  enemy_true: 0.03,
  enemies: [
    { champ: 238, level: 13, armor: 96, mr: 52 },
    { champ: 145, level: 12, armor: 71, mr: 45 },
    { champ: 254, level: 11, armor: 118, mr: 58 },
    { champ: 122, level: 11, armor: 142, mr: 61 },
    { champ: 111, level: 10, armor: 165, mr: 84 },
  ],
  pen_notes: ['Their MR ~60 (Nautilus ~84): % magic pen beats flat magic pen'],
};

export const mayhemDefense: Defense = {
  ehp_physical: 2480,
  ehp_magic: 1890,
  enemy_magic: 0.65,
  enemy_true: 0.06,
  enemies: [
    { champ: 55, level: 12, armor: 103, mr: 66 },
    { champ: 11, level: 11, armor: 98, mr: 63 },
    { champ: 36, level: 11, armor: 171, mr: 92 },
    { champ: 16, level: 10, armor: 61, mr: 41 },
    { champ: 22, level: 10, armor: 58, mr: 38 },
  ],
  pen_notes: ['Their MR ~62 (Dr. Mundo ~92): % magic pen beats flat magic pen'],
};

export const mayhemPicks: MyAugment[] = [
  { id: 2132, candidates: [2132], time: 64 },
  { id: null, candidates: [1030, 1053], time: 402 },
];

export const fullBuildSinks: GoldSink[] = [
  { kind: 'anvil', item: 6032, sell: null, cost: 750, reason: 'Permanent stats, no item slot (a free Cappa Juice with each)' },
  { kind: 'elixir', item: 2139, sell: null, cost: 500, reason: 'Elixir of Sorcery: temporary stats for the next fights, one elixir at a time' },
  { kind: 'sell_boots', item: 3102, sell: 3020, cost: 2230, reason: 'Leg Day covers move speed: sell Sorcerer’s Shoes for Banshee’s Veil' },
  {
    kind: 'swap',
    item: 3157,
    sell: 4645,
    cost: 1010,
    reason: 'Zhonya’s Hourglass over Shadowflame · Anti-burst: Katarina 13/3, killed you 4×',
  },
];
