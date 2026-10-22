// Realistic AppState samples, one per phase/screen, for UI work without the backend.
import type { AppState } from '../lib/bindings/AppState';
import type { GameAdvice } from '../lib/bindings/GameAdvice';
import type { Recap } from '../lib/bindings/Recap';
import type { Settings } from '../lib/bindings/Settings';
import type { Sources } from '../lib/bindings/Sources';
import { mayhemGold, riftGold } from './gold';

export const settings: Settings = { tier: 'emerald_plus', region: 'all', ocr_enabled: true, record_fixtures: false };

const src = (p: Partial<Sources>): Sources => ({
  client: 'ok',
  game: 'off',
  lolalytics: 'ok',
  ugg: 'ok',
  ocr: 'off',
  message: null,
  ...p,
});

const recap: Recap = {
  stats: {
    champ: 134, // Syndra
    mode: 'rift',
    win: false,
    duration_s: 1874,
    kills: 6,
    deaths: 7,
    assists: 9,
    cs: 214,
    gold: 12840,
    damage_to_champs: 28410,
    damage_taken: 21930,
    vision: 19,
    items: [6655, 3020, 4645, 3089, 1058, 3340],
  },
  benchmarks: { cs_per_min: 7.4, gold_per_min: 440, damage_per_min: 960, vision_per_min: 0.75 },
  recommended: [6655, 3020, 3157, 4645, 3089, 3135],
  notes: [
    'Zed killed you 4×; Zhonya’s was recommended from 14:10 and never bought.',
    'CS fell behind after 12 minutes (−31 at 25:00).',
    'Vision 19 vs 23 average for mid Syndra at this length.',
  ],
};

const base = { settings, champ_select: null, game: null, recap: null } satisfies Partial<AppState>;

const riftGame: GameAdvice = {
  time: 1112,
  mode: 'rift',
  my_champ: 134,
  buy_next: {
    item: 3157,
    name: 'Zhonya’s Hourglass',
    cost: 3250,
    score: 0.86,
    reason: 'Zed is 7/1 and killed you 3×',
    component_now: 2420,
  },
  alternatives: [
    { item: 3102, name: 'Banshee’s Veil', cost: 3000, score: 0.71, reason: 'Blocks Nautilus hook and Vi ult engage', component_now: null },
    { item: 3135, name: 'Void Staff', cost: 3000, score: 0.64, reason: 'Darius has 2 MR items', component_now: 1026 },
  ],
  path: [
    { item: 3157, name: 'Zhonya’s Hourglass', cost: 3250, score: 0.86, reason: '', component_now: null },
    { item: 3089, name: 'Rabadon’s Deathcap', cost: 3500, score: 0.8, reason: '', component_now: null },
    { item: 3135, name: 'Void Staff', cost: 3000, score: 0.62, reason: '', component_now: null },
  ],
  owned: [6655, 3020, 4645],
  threats: [
    { champ: 238, name: 'Zed', score: 92, kills: 7, deaths: 1, assists: 2, level_diff: 2, item_gold_diff: 2100, kills_on_me: 3, lane_opp: true, damage: 'AD', flags: ['assassin', 'burst'], legendaries: 2, legendary_diff: 1, new_items: [3142], ult: { rank: 2, cooldowns: [120, 100, 80], spike: true } },
    { champ: 145, name: 'Kai’Sa', score: 71, kills: 5, deaths: 2, assists: 4, level_diff: 1, item_gold_diff: 900, kills_on_me: 0, lane_opp: false, damage: 'Mixed', flags: ['on-hit', 'dive'], legendaries: 2, legendary_diff: 1, new_items: [], ult: { rank: 2, cooldowns: [130, 100, 70], spike: false } },
    { champ: 254, name: 'Vi', score: 48, kills: 3, deaths: 3, assists: 6, level_diff: 0, item_gold_diff: 150, kills_on_me: 1, lane_opp: false, damage: 'AD', flags: ['cc', 'dive'], legendaries: 1, legendary_diff: 0, new_items: [], ult: { rank: 1, cooldowns: [120, 100, 80], spike: false } },
    { champ: 122, name: 'Darius', score: 35, kills: 2, deaths: 4, assists: 1, level_diff: -1, item_gold_diff: -400, kills_on_me: 0, lane_opp: false, damage: 'AD', flags: ['heals', 'tank'], legendaries: 1, legendary_diff: 0, new_items: [], ult: { rank: 1, cooldowns: [120, 100, 80], spike: false } },
    { champ: 111, name: 'Nautilus', score: 22, kills: 0, deaths: 3, assists: 8, level_diff: -2, item_gold_diff: -1200, kills_on_me: 0, lane_opp: false, damage: 'Mixed', flags: ['cc', 'tank'], legendaries: 0, legendary_diff: -1, new_items: [], ult: { rank: 1, cooldowns: [120, 100, 80], spike: false } },
  ],
  matchups: [
    { champ: 238, vs_wr: 46.8, games: 18420, lane_opp: true, label: 'unfavoured' },
    { champ: 145, vs_wr: 50.4, games: 9210, lane_opp: false, label: 'even' },
    { champ: 254, vs_wr: 51.9, games: 7730, lane_opp: false, label: 'favoured' },
    { champ: 122, vs_wr: 49.6, games: 4120, lane_opp: false, label: 'even' },
    { champ: 111, vs_wr: 52.3, games: 6610, lane_opp: false, label: 'favoured' },
  ],
  me: {
    cs_per_min: 6.4,
    cs_benchmark: 7.3,
    cs_diff: -16,
    item_gold_vs_opp: -2100,
    level_vs_opp: -2,
    deaths: 4,
    killers: [
      [238, 3],
      [254, 1],
    ],
    vision: 14,
    vision_benchmark: 17,
  },
  timers: [
    { name: 'Dragon', next_spawn: 1215 },
    { name: 'Baron', next_spawn: 1200 },
    { name: 'Atakhan', next_spawn: null },
  ],
  augments: null,
  gold: riftGold,
  shopping: {
    gold: 1700,
    buys: [{ item: 2420, name: 'Seeker’s Armguard', price: 1600, kind: 'component', toward: 3157 }],
    leftover: 100,
    dead: false,
    respawn_in: 0,
  },
  gold_eta: {
    item: 3157,
    per_min: 412,
    missing: 1550,
    eta_s: 226,
    backs: [
      { gold: 2450, in_s: 109, items: [2420, 1026] },
      { gold: 2900, in_s: 175, items: [2420, 1026, 1052] },
    ],
  },
  deaths: {
    enemies: [
      { champ: 254, in_s: 14 },
      { champ: 111, in_s: 22 },
    ],
    allies: [{ champ: 222, in_s: 8 }],
  },
};

const mayhemGame: GameAdvice = {
  time: 412,
  mode: 'mayhem',
  my_champ: 103,
  buy_next: {
    item: 3089,
    name: 'Rabadon’s Deathcap',
    cost: 3500,
    score: 0.78,
    reason: 'Highest win rate 3rd item on ARAM Ahri',
    component_now: 1058,
  },
  alternatives: [
    { item: 3157, name: 'Zhonya’s Hourglass', cost: 3250, score: 0.7, reason: 'Katarina and Master Yi are fed', component_now: 2420 },
    { item: 3165, name: 'Morellonomicon', cost: 2850, score: 0.61, reason: 'Soraka and Dr. Mundo heal a lot', component_now: 3916 },
  ],
  path: [
    { item: 3089, name: 'Rabadon’s Deathcap', cost: 3500, score: 0.78, reason: '', component_now: null },
    { item: 3135, name: 'Void Staff', cost: 3000, score: 0.6, reason: '', component_now: null },
    { item: 3157, name: 'Zhonya’s Hourglass', cost: 3250, score: 0.58, reason: '', component_now: null },
  ],
  owned: [6655, 3020, 4645],
  threats: [
    { champ: 55, name: 'Katarina', score: 84, kills: 9, deaths: 3, assists: 5, level_diff: 1, item_gold_diff: 1600, kills_on_me: 2, lane_opp: false, damage: 'AP', flags: ['assassin', 'burst'], legendaries: 3, legendary_diff: 1, new_items: [3089], ult: { rank: 2, cooldowns: [60, 52.5, 45], spike: false } },
    { champ: 11, name: 'Master Yi', score: 66, kills: 6, deaths: 4, assists: 2, level_diff: 0, item_gold_diff: 800, kills_on_me: 1, lane_opp: false, damage: 'Mixed', flags: ['on-hit', 'true'], legendaries: 2, legendary_diff: 0, new_items: [], ult: { rank: 2, cooldowns: [85, 85, 85], spike: false } },
    { champ: 36, name: 'Dr. Mundo', score: 41, kills: 2, deaths: 5, assists: 7, level_diff: 0, item_gold_diff: 300, kills_on_me: 0, lane_opp: false, damage: 'AD', flags: ['tank', 'heals'], legendaries: 2, legendary_diff: 0, new_items: [], ult: { rank: 1, cooldowns: [110, 100, 90], spike: true } },
    { champ: 16, name: 'Soraka', score: 30, kills: 1, deaths: 4, assists: 14, level_diff: -1, item_gold_diff: -500, kills_on_me: 0, lane_opp: false, damage: 'AP', flags: ['heals'], legendaries: 1, legendary_diff: -1, new_items: [], ult: { rank: 1, cooldowns: [160, 145, 130], spike: false } },
    { champ: 22, name: 'Ashe', score: 27, kills: 3, deaths: 6, assists: 8, level_diff: -1, item_gold_diff: -700, kills_on_me: 0, lane_opp: false, damage: 'AD', flags: ['cc', 'crit'], legendaries: 1, legendary_diff: -1, new_items: [], ult: { rank: 1, cooldowns: [100, 80, 60], spike: false } },
  ],
  matchups: [],
  me: {
    cs_per_min: 4.1,
    cs_benchmark: 3.6,
    cs_diff: 3,
    item_gold_vs_opp: null,
    level_vs_opp: null,
    deaths: 4,
    killers: [
      [55, 2],
      [11, 1],
      [22, 1],
    ],
    vision: 0,
    vision_benchmark: null,
  },
  timers: [],
  augments: {
    offered: [
      { id: 1053, name: 'Mad Scientist', rarity: 'prismatic', rank: 5, of: 52 },
      { id: 1030, name: 'Eureka', rarity: 'prismatic', rank: 0, of: 52 },
      { id: 1243, name: 'Transmute: Chaos', rarity: 'prismatic', rank: 9, of: 52 },
    ],
    best: 1030,
    reason: 'Eureka is Ahri’s #1 prismatic on u.gg',
  },
  gold: mayhemGold,
  shopping: {
    gold: 2600,
    buys: [
      { item: 1058, name: 'Needlessly Large Rod', price: 1200, kind: 'component', toward: 3089 },
      { item: 1026, name: 'Blasting Wand', price: 850, kind: 'component', toward: 3089 },
      { item: 1052, name: 'Amplifying Tome', price: 400, kind: 'component', toward: 3135 },
    ],
    leftover: 150,
    dead: false,
    respawn_in: 0,
  },
  gold_eta: { item: 3089, per_min: 655, missing: 900, eta_s: 82, backs: [{ gold: 2700, in_s: 9, items: [1058, 1026, 1026] }] },
  deaths: { enemies: [], allies: [] },
};

// Same Mayhem game, a minute later: I died with 3720 g, enemies are dead too.
const mayhemDead: GameAdvice = {
  ...mayhemGame,
  time: 475,
  buy_next: mayhemGame.buy_next && { ...mayhemGame.buy_next, component_now: null },
  shopping: {
    gold: 3720,
    buys: [{ item: 3089, name: 'Rabadon’s Deathcap', price: 3500, kind: 'full', toward: null }],
    leftover: 220,
    dead: true,
    respawn_in: 18,
  },
  gold_eta: { item: 3089, per_min: 655, missing: 0, eta_s: 0, backs: [] },
  deaths: {
    enemies: [
      { champ: 55, in_s: 6 },
      { champ: 22, in_s: 11 },
    ],
    allies: [{ champ: 81, in_s: 3 }],
  },
};

export const fixtures = {
  noclient: {
    ...base,
    phase: 'noclient',
    sources: src({ client: 'off', ugg: 'off', message: 'League client not running' }),
  },
  idle: { ...base, phase: 'idle', recap, sources: src({}) },
  'champselect rift': {
    ...base,
    phase: 'champselect',
    sources: src({ message: 'lolalytics: patch 16.21 sample small, using last 30 days', lolalytics: 'degraded' }),
    champ_select: {
      mode: 'rift',
      my_lane: 'middle',
      my_champ: 134,
      picks: [
        { champ: 134, score: 0.62, reason: '53.1% vs Zed (lane) · synergy with Lee Sin +2.2%' },
        { champ: 1, score: 0.58, reason: 'adds magic damage: team is 86% physical · 54.0% vs Zed (lane)' },
        { champ: 45, score: 0.55, reason: 'synergy with Jinx +3.1%' },
      ],
      bans: [
        { champ: 238, score: 0.7, reason: 'Hard counter to Syndra (46.8%), 12% pick rate' },
        { champ: 112, score: 0.52, reason: 'Meta threat mid: 53.2%, 9% pick rate' },
        { champ: 4, score: 0.44, reason: 'Beats Syndra 52.9%' },
      ],
      build: {
        start: [1056, 2003, 2003],
        core: [
          { item: 6655, name: 'Luden’s Echo', cost: 2750, score: 0.8, reason: 'Most built first item (61%)', component_now: null },
          { item: 4645, name: 'Shadowflame', cost: 3200, score: 0.72, reason: '54.2% as 2nd item', component_now: null },
          { item: 3089, name: 'Rabadon’s Deathcap', cost: 3500, score: 0.7, reason: 'Best 3rd item', component_now: null },
        ],
        boots: { item: 3020, name: 'Sorcerer’s Shoes', cost: 1100, score: 0.6, reason: 'No heavy CC revealed yet', component_now: null },
      },
      bench: [],
      enemies: [238, 145, 254],
      draft: {
        lanes: [
          { lane: 'top', ally: 86, enemy: null, ally_wr: null, games: 0 },
          { lane: 'jungle', ally: 64, enemy: 254, ally_wr: 51.6, games: 802 },
          { lane: 'middle', ally: 134, enemy: 238, ally_wr: 53.1, games: 2140 },
          { lane: 'bottom', ally: 222, enemy: 145, ally_wr: 47.8, games: 3311 },
        ],
        allies: {
          champs: [134, 86, 64, 222],
          physical: 0.56,
          magic: 0.4,
          true_dmg: 0.04,
          frontline: 1,
          engage: 0,
          cc: 1.6,
          duos: [
            { a: 134, b: 64, delta: 2.2, games: 1240 },
            { a: 64, b: 222, delta: -0.8, games: 980 },
          ],
          gaps: ['No engage'],
          factors: [
            { label: 'Lanes', points: 1.9, detail: 'Jungle 51.6%, Mid 53.1%, Bot 47.8%' },
            { label: 'Champions', points: 1.8, detail: 'Garen 52.0%' },
            { label: 'Synergy', points: 1.1, detail: 'Syndra + Lee Sin +2.2' },
            { label: 'Composition', points: -0.5, detail: 'No engage' },
          ],
          total: 4.3,
        },
        enemies: {
          champs: [238, 145, 254],
          physical: 0.86,
          magic: 0.14,
          true_dmg: 0,
          frontline: 0,
          engage: 1,
          cc: 1.3,
          duos: [{ a: 238, b: 254, delta: -1.6, games: 610 }],
          gaps: ['No frontline', 'Mostly physical (86%)'],
          factors: [
            { label: 'Lanes', points: -1.9, detail: 'Jungle 48.4%, Mid 46.9%, Bot 52.2%' },
            { label: 'Synergy', points: -0.4, detail: 'Zed + Vi −1.6' },
            { label: 'Composition', points: -2.0, detail: 'No frontline, Mostly physical (86%)' },
          ],
          total: -4.3,
        },
      },
    },
  },
  'champselect mayhem': {
    ...base,
    phase: 'champselect',
    sources: src({}),
    champ_select: {
      mode: 'mayhem',
      my_lane: null,
      my_champ: 81,
      picks: [],
      bans: [],
      build: {
        start: [1052, 2003],
        core: [
          { item: 3078, name: 'Trinity Force', cost: 3333, score: 0.74, reason: 'Most built first item on ARAM', component_now: null },
          { item: 3004, name: 'Manamune', cost: 2900, score: 0.7, reason: '55.0% as 2nd item', component_now: null },
          { item: 3161, name: 'Spear of Shojin', cost: 3100, score: 0.62, reason: '', component_now: null },
        ],
        boots: { item: 3158, name: 'Ionian Boots of Lucidity', cost: 900, score: 0.6, reason: 'Haste for spam', component_now: null },
      },
      bench: [
        { champ: 99, tier: 'S+', mine: false },
        { champ: 103, tier: 'S', mine: false },
        { champ: 81, tier: 'A', mine: true },
        { champ: 86, tier: 'B', mine: false },
        { champ: 67, tier: 'C', mine: false },
      ],
      enemies: [],
      draft: null,
    },
  },
  'ingame rift': { ...base, phase: 'ingame', sources: src({ game: 'ok' }), game: riftGame },
  'ingame mayhem': { ...base, phase: 'ingame', sources: src({ game: 'ok', ocr: 'ok' }), game: mayhemGame },
  'ingame dead': { ...base, phase: 'ingame', sources: src({ game: 'ok', ocr: 'ok' }), game: mayhemDead },
  loading: { ...base, phase: 'loading', sources: src({ game: 'degraded', message: 'Game is loading' }) },
  postgame: { ...base, phase: 'postgame', recap, sources: src({}) },
} satisfies Record<string, AppState>;

export type FixtureName = keyof typeof fixtures;
