// Backend access: Tauri commands/events inside the app, an in-memory mock in a plain browser.
import type { AppState } from './bindings/AppState';
import type { ChampStats } from './bindings/ChampStats';
import type { Lane } from './bindings/Lane';
import type { MatchAnalysis } from './bindings/MatchAnalysis';
import type { Mode } from './bindings/Mode';
import type { Settings } from './bindings/Settings';
import type { UiStatic } from './bindings/UiStatic';
import type { TftSetData } from './tft-bindings/TftSetData';
import type { TftState } from './tft-bindings/TftState';

export interface Api {
  getState(): Promise<AppState>;
  getStatic(): Promise<UiStatic>;
  setSettings(settings: Settings): Promise<AppState>;
  lookup(champ: number, lane: Lane | null, mode: Mode): Promise<ChampStats>;
  /** Review one game from the match history (needs the League client). */
  analyzeGame(gameId: number): Promise<MatchAnalysis>;
  /** Subscribe to pushed state; returns an unsubscribe function. */
  onState(cb: (s: AppState) => void): Promise<() => void>;
  /** Current TFT state (phase "none" outside TFT). */
  getTft(): Promise<TftState>;
  /** Community Dragon Set data: display names and icon paths for units, traits, items, augments. */
  getTftStatic(): Promise<TftSetData>;
  /** Subscribe to pushed TFT state (its own event, apart from AppState); returns an unsubscribe function. */
  onTft(cb: (s: TftState) => void): Promise<() => void>;
}

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function tauriApi(): Promise<Api> {
  const [{ invoke }, { listen }] = await Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')]);
  return {
    getState: () => invoke('get_state'),
    getStatic: () => invoke('get_static'),
    setSettings: (settings) => invoke('set_settings', { settings }),
    lookup: (champ, lane, mode) => invoke('lookup', { champ, lane, mode }),
    analyzeGame: (gameId) => invoke('analyze_game', { gameId }),
    onState: (cb) => listen<AppState>('state', (e) => cb(e.payload)),
    getTft: () => invoke('get_tft'),
    getTftStatic: () => invoke('get_tft_static'),
    onTft: (cb) => listen<TftState>('tft', (e) => cb(e.payload)),
  };
}

export const api: Promise<Api> = isTauri ? tauriApi() : import('../mock').then((m) => m.mockApi);
