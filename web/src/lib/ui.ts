// Names, icon URLs and number formatting shared by components.
import type { Lane } from './bindings/Lane';
import type { Mode } from './bindings/Mode';
import type { UiStatic } from './bindings/UiStatic';

const DD = 'https://ddragon.leagueoflegends.com/cdn';

export const champIcon = (ui: UiStatic, key: number) => {
  const c = ui.champs[key];
  return c ? `${DD}/${ui.version}/img/champion/${c.id}.png` : '';
};
export const itemIcon = (ui: UiStatic, id: number) => `${DD}/${ui.version}/img/item/${id}.png`;
export const augmentIcon = (ui: UiStatic, id: number) =>
  ui.ugg_patch ? `https://static.bigbrain.gg/cdragon-custom/${ui.ugg_patch}/augments/${id}.webp` : '';

/** Rune / stat shard image path from Data Dragon (`perk-images/…`); not versioned. */
export const runeIcon = (path: string) => (path ? `${DD}/img/${path}` : '');

export const champName = (ui: UiStatic, key: number) => ui.champs[key]?.name ?? `#${key}`;
export const itemName = (ui: UiStatic, id: number) => ui.items[id]?.name ?? `Item ${id}`;

/** 2140 → "2.1k", 850 → "850". */
export const gold = (g: number) => (Math.abs(g) >= 1000 ? `${(g / 1000).toFixed(1)}k` : `${Math.round(g)}`);
/** Signed with a real minus sign. */
export const signed = (n: number, fmt: (n: number) => string = (x) => `${Math.round(x)}`) =>
  n > 0 ? `+${fmt(n)}` : n < 0 ? `−${fmt(-n)}` : '±0';
export const pct = (n: number, digits = 1) => `${n.toFixed(digits)}%`;
export const clockText = (s: number) => {
  const t = Math.max(0, Math.floor(s));
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
};

export const LANES: Lane[] = ['top', 'jungle', 'middle', 'bottom', 'support'];
export const LANE_NAMES: Record<Lane, string> = {
  top: 'Top',
  jungle: 'Jungle',
  middle: 'Mid',
  bottom: 'Bot',
  support: 'Support',
};
export const MODE_NAMES: Record<Mode, string> = { rift: "Summoner's Rift", aram: 'ARAM', mayhem: 'ARAM: Mayhem' };
