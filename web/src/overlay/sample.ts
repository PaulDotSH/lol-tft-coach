// What the overlay page shows in a plain browser (`npm --prefix web run dev`,
// /overlay.html?part=strip or ?part=cards; badge x as shares of the card band).
import type { Overlay } from '../lib/bindings/Overlay';

export const sample: Overlay = {
  tft: false,
  badge_y: 0.33,
  badges: [
    { x: 0.2333, place: 1, reason: '#4 of 21 Gold' },
    { x: 0.5, place: 0, reason: "fits your Nashor's Tooth (on-hit)" },
    { x: 0.7667, place: null, reason: null },
  ],
  head: "Buy Rabadon's Deathcap",
  detail: '450 g short · ~40 s',
  call: 'Baron in 40 s: back now and buy',
  time: 1210,
  timers: [
    { name: 'Dragon', next_spawn: 1290 },
    { name: 'Baron', next_spawn: 1250 },
    { name: 'Herald', next_spawn: null },
  ],
};
