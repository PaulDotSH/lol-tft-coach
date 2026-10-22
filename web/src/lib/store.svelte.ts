// Single source of UI state. AppState is replaced wholesale on every push, so it is held
// raw (no deep proxy); components derive what they need and keyed each blocks keep DOM stable.
import { api } from './api';
import type { AppState } from './bindings/AppState';
import type { Settings } from './bindings/Settings';
import type { UiStatic } from './bindings/UiStatic';

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
let anchor = { time: 0, at: 0 };
let now = $state(performance.now());
export const clock = {
  get time() {
    return anchor.time + (now - anchor.at) / 1000;
  },
};

function apply(s: AppState) {
  if (s.game && s.game.time !== anchor.time) anchor = { time: s.game.time, at: performance.now() };
  state = s;
}

export async function start() {
  setInterval(() => (now = performance.now()), 1000);
  try {
    const a = await api;
    const [u, s] = await Promise.all([a.getStatic(), a.getState()]);
    ui = u;
    apply(s);
    await a.onState(apply);
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
