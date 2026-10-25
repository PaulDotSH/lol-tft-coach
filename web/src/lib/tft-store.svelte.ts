// TFT state, pushed on its own "tft" event (apart from the LoL AppState), and the Set data
// for names and icons. Both are replaced wholesale, so they are held raw like AppState.
import { api } from './api';
import type { TftSetData } from './tft-bindings/TftSetData';
import type { TftState } from './tft-bindings/TftState';

let state = $state.raw<TftState | null>(null);
let set = $state.raw<TftSetData | null>(null);

export const tft = {
  get state() {
    return state;
  },
  get set() {
    return set;
  },
};

/** Set data loads only once a TFT phase starts (it is a big download for LoL-only sessions). */
let loading = false;
async function loadSet() {
  if (set || loading) return;
  loading = true;
  const a = await api;
  while (!set && state && state.phase !== 'none') {
    try {
      set = await a.getTftStatic();
    } catch (e) {
      console.warn('TFT set data not available yet:', e);
      await new Promise((r) => setTimeout(r, 10_000));
    }
  }
  loading = false;
}

function apply(s: TftState) {
  state = s;
  if (s.phase !== 'none') loadSet();
}

export async function startTft() {
  const a = await api;
  try {
    await a.onTft(apply);
  } catch (e) {
    console.warn('TFT events unavailable:', e);
  }
  // Pushes carry everything; the first read only covers a TFT phase already running at startup.
  a.getTft().then(
    (s) => !state && apply(s),
    () => {},
  );
}
