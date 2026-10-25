// Game clock between pushes: the last pushed game time plus the time elapsed since it arrived.
// Plain TS without imports, so `node --test` runs its test without a bundler.

/** Game time `time` (seconds) as pushed at `at` (performance.now() ms). */
export type Anchor = { time: number; at: number };

/** Game seconds at `nowMs`. Pass the real performance.now() of the read, never a sampled tick: a
 *  throttled interval (window behind the game) leaves a tick far older than the last push, which put
 *  the clock that far behind and every countdown that far too long. */
export const gameTime = (a: Anchor, nowMs: number) => a.time + Math.max(0, nowMs - a.at) / 1000;
