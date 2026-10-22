// In-memory backend for a plain browser (no Tauri). Pushes the selected fixture every 2 s,
// advancing the game clock like the real orchestrator would.
import type { Api } from '../lib/api';
import type { AppState } from '../lib/bindings/AppState';
import type { ChampStats } from '../lib/bindings/ChampStats';
import type { ItemStat } from '../lib/bindings/ItemStat';
import { fixtures, settings as defaultSettings, type FixtureName } from './states';
import { uiStatic } from './static';

// `?mock=idle` (etc.) picks the starting fixture; the dev switcher changes it live.
const fromUrl = new URLSearchParams(location.search).get('mock') as FixtureName | null;
let current: FixtureName = fromUrl && fromUrl in fixtures ? fromUrl : 'ingame rift';
let settings = defaultSettings;
let elapsed = 0;
const subs = new Set<(s: AppState) => void>();

function snapshot(): AppState {
  const f: AppState = fixtures[current];
  const game = f.game && { ...f.game, time: f.game.time + elapsed };
  return { ...f, game, settings };
}
const push = () => subs.forEach((cb) => cb(snapshot()));

setInterval(() => {
  if (fixtures[current].game) {
    elapsed += 2;
    push();
  }
}, 2000);

export const mockFixtures = Object.keys(fixtures) as FixtureName[];
export const mockPhase = {
  get: () => current,
  set(name: FixtureName) {
    current = name;
    elapsed = 0;
    push();
  },
};

// Deterministic pseudo-random so a lookup always renders the same numbers.
function rng(seed: number) {
  return () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
}

const AP = [6655, 3157, 3089, 4645, 3135, 3165, 3102, 6653, 4646, 3003, 3040];
const AD = [3031, 3072, 3036, 3033, 3139, 3026, 6672, 3094, 3085, 3046, 3142, 6676, 3814];
const BRUISER = [3078, 6610, 3071, 6333, 3053, 3748, 6698, 3074, 3161, 3065, 3742, 4401];
const BOOTS = [3020, 3047, 3111, 3006, 3158, 3009];

function lookup(champ: number, lane: ChampStats['lane'], mode: ChampStats['mode']): ChampStats {
  const r = rng(champ * 7 + (lane?.length ?? 0) + mode.length);
  const pool = [AP, AD, BRUISER][champ % 3];
  const stat = (id: number, n: number): ItemStat => ({ id, games: n, wins: Math.round(n * (0.45 + r() * 0.12)) });
  const pick = (list: number[], k: number) => [...list].sort(() => r() - 0.5).slice(0, k);
  const keys = Object.keys(uiStatic.champs).map(Number).filter((k) => k !== champ);
  const counters = pick(keys, 24).map((c) => ({
    champ: c,
    vs_wr: 44 + r() * 12,
    games: Math.round(500 + r() * 9000),
    default_lane: lane,
  }));
  return {
    champ,
    lane,
    mode,
    patch: r() > 0.3 ? '16.21' : '30',
    win_rate: 47 + r() * 7,
    pick_rate: 1 + r() * 12,
    games: Math.round(20000 + r() * 200000),
    lanes: mode === 'rift' ? { [lane ?? 'middle']: 82, top: 9, support: 4 } : {},
    slots: [0, 1, 2, 3, 4].map((s) => pick(pool, 3).map((id, i) => stat(id, Math.round(40000 / (s + 1) / (i + 1))))),
    boots: pick(BOOTS, 3).map((id, i) => stat(id, Math.round(30000 / (i + 1)))),
    early: [
      { items: [1056, 2003, 2003], win_rate: 51.8, pick_rate: 64.2, games: 81200 },
      { items: [1055, 2003], win_rate: 50.9, pick_rate: 21.7, games: 27400 },
      { items: [1054, 2003], win_rate: 49.7, pick_rate: 8.1, games: 10300 },
    ],
    counters,
    benchmarks: { cs_per_min: 6 + r() * 2, gold_per_min: 380 + r() * 80, damage_per_min: 700 + r() * 400, vision_per_min: 0.6 + r() * 0.5 },
  };
}

const delay = <T>(v: T, ms = 120) => new Promise<T>((res) => setTimeout(() => res(v), ms));

export const mockApi: Api = {
  getState: () => delay(snapshot()),
  getStatic: () => delay(uiStatic),
  setSettings(s) {
    settings = s;
    return delay(snapshot());
  },
  lookup: (champ, lane, mode) => delay(lookup(champ, lane, mode), 300),
  async onState(cb) {
    subs.add(cb);
    return () => subs.delete(cb);
  },
};
