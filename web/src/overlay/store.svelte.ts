// Overlay view from the backend (`overlay` event, `get_overlay` on load) and a game clock
// that keeps the objective timers counting between pushes.
import type { Overlay } from '../lib/bindings/Overlay';
import { type Anchor, gameTime } from '../lib/clock';
import { sample } from './sample';

let view = $state.raw<Overlay | null>(null);
let anchor: Anchor = { time: 0, at: 0 };
let tick = $state(0);

export const overlay = {
  get view() {
    return view;
  },
  get time() {
    void tick;
    return gameTime(anchor, performance.now());
  },
};

function apply(v: Overlay | null) {
  if (v && v.time !== anchor.time) anchor = { time: v.time, at: performance.now() };
  view = v;
}

export async function startOverlay() {
  // Only the strip counts timers down; the card band redraws on pushes alone.
  if (new URLSearchParams(location.search).get('part') !== 'cards') setInterval(() => tick++, 1000);
  if (!('__TAURI_INTERNALS__' in window)) return apply(sample);
  const [{ invoke }, { listen }] = await Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')]);
  await listen<Overlay | null>('overlay', (e) => apply(e.payload));
  apply(await invoke<Overlay | null>('get_overlay'));
}
