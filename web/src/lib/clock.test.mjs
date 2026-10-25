// `npm test` (node --test; Node 24 strips the types of clock.ts itself).
import assert from 'node:assert/strict';
import test from 'node:test';
import { gameTime } from './clock.ts';

test('counts on from the last push', () => {
  assert.equal(gameTime({ time: 600, at: 10_000 }, 10_000), 600);
  assert.equal(gameTime({ time: 600, at: 10_000 }, 30_000), 620);
});

test('a read older than the push never puts the clock behind it', () => {
  // The interval tick last ran 20 s before this push (throttled behind the game): the old clock
  // read 600 − 20 = 580 here, and a dragon at 1:10 showed 1:30.
  const dragon = 670;
  const a = { time: 600, at: 50_000 };
  assert.equal(dragon - gameTime(a, 30_000), 70);
  assert.equal(dragon - gameTime(a, 50_500), 69.5);
});
