import assert from 'node:assert/strict';
import test from 'node:test';
import { speakable, trendMark, winPath } from './winprob.ts';

const call = (key) => ({ key, text: key, say: key });
const s = (time, win) => ({ time, team: 0, lane: null, win });

test('a stance change is spoken only with posture calls on', () => {
  const calls = [call('back-Baron-1200'), call('stance-press-1'), call('gank-top-3')];
  assert.deepEqual(speakable(calls, false).map((c) => c.key), ['back-Baron-1200', 'gank-top-3']);
  assert.deepEqual(speakable(calls, true).map((c) => c.key), ['back-Baron-1200', 'stance-press-1', 'gank-top-3']);
});

test('win line spans the whole history, 50 % on the middle line', () => {
  assert.equal(winPath([s(0, null), s(20, null)], 200, 56), null);
  assert.equal(winPath([s(0, null), s(20, 0.6)], 200, 56), null);
  assert.equal(winPath([s(0, null), s(50, 0.5), s(100, 1)], 200, 56), 'M100.0,28.0L200.0,2.0');
  assert.equal(winPath([s(0, 0), s(100, 0.25)], 200, 56), 'M0.0,54.0L200.0,41.0');
  assert.equal(winPath([s(0, 0.3), s(100, 0.6)], 200, 56, 0.2), 'M0.0,54.0L200.0,15.0');
});

test('trend arrow moves only past a point', () => {
  assert.equal(trendMark(4.2), '↑');
  assert.equal(trendMark(-1.5), '↓');
  assert.equal(trendMark(0.6), '→');
  assert.equal(trendMark(-0.9), '→');
});
