import type { Call } from './bindings/Call';
import type { GoldSample } from './bindings/GoldSample';

/** Calls to speak: a stance change (`stance-…`) only with the posture calls setting on. */
export const speakable = (calls: Call[], postureCalls: boolean) => calls.filter((c) => postureCalls || !c.key.startsWith('stance-'));

/** `GoldSample.win` over the game as an SVG path `w` × `h`, x over the whole history, 50 % on the
 *  middle line and 50 % ± `half` at the edges; null under two samples that carry it. */
export function winPath(history: GoldSample[], w: number, h: number, half = 0.5): string | null {
  const pts = history.filter((s) => s.win != null);
  if (pts.length < 2) return null;
  const t0 = history[0].time;
  const span = Math.max(history[history.length - 1].time - t0, 1);
  return pts
    .map((s, i) => `${i ? 'L' : 'M'}${(((s.time - t0) / span) * w).toFixed(1)},${(h / 2 - ((s.win! - 0.5) / half) * (h / 2 - 2)).toFixed(1)}`)
    .join('');
}

/** Two-minute trend in points → arrow; flat within a point. */
export const trendMark = (t: number) => (t >= 1 ? '↑' : t <= -1 ? '↓' : '→');
