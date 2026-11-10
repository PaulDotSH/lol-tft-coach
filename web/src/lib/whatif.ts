import type { WhatIf } from './bindings/WhatIf';

const SHOWN = 0.005;

export const gainText = (g: number) => {
  const n = Math.round(g * 100);
  return n > 0 ? `+${n} %` : n < 0 ? `−${-n} %` : '±0 %';
};

export function figure(w: WhatIf | undefined): string {
  if (!w) return '';
  const parts = [];
  if (w.damage >= SHOWN) parts.push(`${gainText(w.damage)} dmg`);
  if (w.ehp >= SHOWN) parts.push(`${gainText(w.ehp)} EHP`);
  if (!parts.length) return '';
  return (w.rough ? '~' : '') + parts.join(' · ');
}

export function ehpLine(w: WhatIf): string {
  if (w.ehp < SHOWN) return '';
  return `${gainText(w.ehp)} EHP vs their mix (${gainText(w.ehp_magic)} vs magic, ${gainText(w.ehp_physical)} vs physical)`;
}
