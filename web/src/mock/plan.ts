// Game-plan card samples: a Rift draft (Syndra mid into Zed, Kayle top on their side).
import type { GamePlan } from '../lib/bindings/GamePlan';

export const riftPlan: GamePlan = {
  phases: [
    { name: 'Early', from_min: 0, to_min: 25, edge: 1.4, even: false },
    { name: 'Mid', from_min: 25, to_min: 35, edge: -0.3, even: true },
    { name: 'Late', from_min: 35, to_min: null, edge: -2.1, even: false },
  ],
  scalers: [
    { champ: 64, ally: true, late: -1.3, early: 1.9, off_lane: false },
    { champ: 222, ally: true, late: 1.7, early: -0.8, off_lane: false },
    { champ: 10, ally: false, late: 4.2, early: -3.6, off_lane: false },
    { champ: 254, ally: false, late: -0.6, early: 1.6, off_lane: true },
  ],
  lines: [
    { kind: 'scaling', text: 'They are stronger late (+2.1 after 35 min: Kayle, Kai\'Sa). You are stronger early (+1.4 before 25 min: Lee Sin).' },
    { kind: 'comp', text: 'Engage: you 1, them 2. Frontline: you 1, them 1. Their damage is 82% physical. You: no peel for carries.' },
    { kind: 'spikes', text: 'Level 6 spikes on their side: Zed, Kai\'Sa, Nautilus.' },
    { kind: 'lane', text: 'Lane vs Zed: 53.1% for you (2.1k games). Zed: Shadow (W) into E-Q; R all-in at 6 when you are low.' },
    { kind: 'record', text: 'Your record: 2–5 vs Zed, 3–1 vs Kayle.' },
  ],
  fold: 'Stronger early · they scale: Kayle',
  source: 'lolalytics',
  partial: true,
  curves: 9,
  champs: 10,
  show_s: 120,
};
