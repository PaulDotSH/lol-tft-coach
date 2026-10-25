// TFT names, Community Dragon icon URLs and labels shared by the TFT screens.
import type { TftMode } from './tft-bindings/TftMode';
import type { TftSetData } from './tft-bindings/TftSetData';

export type TftKind = 'unit' | 'item' | 'augment' | 'trait';

const CDRAGON = 'https://raw.communitydragon.org/latest/game/';

/** Community Dragon asset path (`ASSETS/…/X.tex`) → image URL. The one place the rule lives. */
export const cdragonUrl = (path: string) => (path ? CDRAGON + path.toLowerCase().replace(/\.tex$/, '.png') : '');

function entry(set: TftSetData | null, kind: TftKind, api: string): { name: string; icon: string } | undefined {
  if (!set) return undefined;
  if (kind === 'unit') return set.units[api];
  if (kind === 'item') return set.items[api];
  if (kind === 'augment') return set.augments[api];
  return set.traits[api];
}

/**
 * Readable fallback for an API name the Set data doesn't know (or before it loads):
 * "DA_18_Ahri" → "Ahri", "DA_Component_BFSword" → "BF Sword". Never shows the raw id.
 */
export function prettify(api: string) {
  return api
    .replace(/^(DA_|TFT\d*_)/, '')
    .replace(/^(Item|Augment|Component|Trait)_/, '')
    .split('_')
    .filter((p) => p && !/^\d+$/.test(p) && p !== 'Base' && p !== 'UniqueTrait')
    .map((p) => p.replace(/\d+$/, '').replace(/^\d+/, ''))
    .join(' ')
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
    .trim();
}

export const tftName = (set: TftSetData | null, kind: TftKind, api: string) => entry(set, kind, api)?.name ?? prettify(api);
export const tftIcon = (set: TftSetData | null, kind: TftKind, api: string) => cdragonUrl(entry(set, kind, api)?.icon ?? '');
export const unitCost = (set: TftSetData | null, api: string) => set?.units[api]?.cost ?? 0;

/** Unit frame colours by cost, as the game draws them. */
export const COST_COLORS: Record<number, string> = {
  1: '#8f9bab',
  2: '#2fb37c',
  3: '#3a8fe0',
  4: '#c25ae0',
  5: '#f0b43c',
};
export const costColor = (cost: number) => COST_COLORS[Math.min(cost, 5)] ?? 'var(--line)';

export const stageText = (stage: [number, number] | null) => (stage ? `${stage[0]}-${stage[1]}` : '');
export const stars = (n: number) => '★'.repeat(Math.max(1, Math.min(n, 3)));

/** 1 → "1st", 4 → "4th". */
export const placeText = (n: number) => `${n}${n === 1 ? 'st' : n === 2 ? 'nd' : n === 3 ? 'rd' : 'th'}`;
/** Class for a placement: win, top 4, bottom 4. */
export const placeClass = (n: number) => (n === 1 ? 'first' : n <= 4 ? 'top4' : 'bot4');
/** Average placement, lower is better: "4.12". */
export const avgText = (n: number) => n.toFixed(2);
/** Share 0..=1 → "54%". */
export const share = (n: number) => `${Math.round(n * 100)}%`;

export const MODE_NAMES: Record<TftMode, string> = { standard: 'Standard', choncc: 'Choncc or Treat' };

const QUEUES: Record<number, string> = {
  1090: 'Normal',
  1100: 'Ranked',
  1230: 'Choncc or Treat',
};
/** Queue arrives as a number from the backend (the binding says bigint; see `queue` in tft-model). */
export const tftQueueName = (queue: number | bigint | null, mode: TftMode) =>
  (queue != null ? QUEUES[Number(queue)] : undefined) ?? MODE_NAMES[mode];
