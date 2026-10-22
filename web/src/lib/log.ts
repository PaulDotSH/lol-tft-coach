// Inside the app, forward frontend errors and warnings to the backend log
// (`logs\lol-coach.log`, plus the debug session folder in debug mode).
import { isTauri } from './api';

export function forwardLogs() {
  if (!isTauri) return;
  const send = (level: 'error' | 'warn', parts: unknown[]) => {
    const message = parts.map((p) => (p instanceof Error ? `${p.message}\n${p.stack ?? ''}` : typeof p === 'string' ? p : safeJson(p))).join(' ');
    import('@tauri-apps/api/core').then(({ invoke }) => invoke('log_frontend', { level, message })).catch(() => {});
  };
  for (const level of ['error', 'warn'] as const) {
    const orig = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      orig(...args);
      send(level, args);
    };
  }
  window.addEventListener('error', (e) => send('error', [e.error ?? e.message]));
  window.addEventListener('unhandledrejection', (e) => send('error', ['unhandled rejection:', e.reason]));
}

function safeJson(v: unknown): string {
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}
