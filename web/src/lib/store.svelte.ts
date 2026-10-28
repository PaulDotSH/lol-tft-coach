// Single source of UI state. AppState is replaced wholesale on every push, so it is held
// raw (no deep proxy); components derive what they need and keyed each blocks keep DOM stable.
import { api } from './api';
import type { AppState } from './bindings/AppState';
import type { Settings } from './bindings/Settings';
import type { UiStatic } from './bindings/UiStatic';
import { type Anchor, gameTime } from './clock';

let state = $state.raw<AppState | null>(null);
let ui = $state.raw<UiStatic | null>(null);
let error = $state<string | null>(null);

export const store = {
  get state() {
    return state;
  },
  get ui() {
    return ui;
  },
  get error() {
    return error;
  },
};

// Game clock that keeps ticking locally between pushes (seconds since game start).
// `tick` only re-renders readers each second; the time itself is read from performance.now().
let anchor: Anchor = { time: 0, at: 0 };
let tick = $state(0);
export const clock = {
  get time() {
    void tick;
    return gameTime(anchor, performance.now());
  },
};

function apply(s: AppState) {
  if (s.game && s.game.time !== anchor.time) anchor = { time: s.game.time, at: performance.now() };
  state = s;
}

export async function start() {
  setInterval(() => tick++, 1000);
  // Throttled while hidden behind the game: catch up as soon as the window shows again.
  document.addEventListener('visibilitychange', () => tick++);
  const a = await api;
  try {
    await a.onState(apply);
    apply(await a.getState());
  } catch (e) {
    error = String(e);
  }
  // Static data needs the first Data Dragon load (or the network): retry until it arrives.
  while (!ui) {
    try {
      ui = await a.getStatic();
      error = null;
    } catch (e) {
      error = String(e);
      await new Promise((r) => setTimeout(r, 3000));
    }
  }
}

export async function coachless(signIn: boolean) {
  try {
    const a = await api;
    await (signIn ? a.coachlessSignIn() : a.coachlessSignOut());
  } catch (e) {
    error = String(e);
  }
}

export async function saveSettings(settings: Settings) {
  try {
    apply(await (await api).setSettings(settings));
  } catch (e) {
    error = String(e);
  }
}
