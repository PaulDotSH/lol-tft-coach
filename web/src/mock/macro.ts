// Macro coach report samples (shape of AppState.macro_report).
import type { MacroReport } from '../lib/bindings/MacroReport';

export const macroReport: MacroReport = {
  games: 27,
  with_timeline: 25,
  modes: [
    { mode: 'rift', games: 14, wins: 6 },
    { mode: 'aram', games: 5, wins: 3 },
    { mode: 'mayhem', games: 8, wins: 4 },
  ],
  champs: [
    { mode: 'rift', champ: 103, games: 9, wins: 3 },
    { mode: 'rift', champ: 134, games: 5, wins: 3 },
    { mode: 'mayhem', champ: 99, games: 3, wins: 2 },
    { mode: 'aram', champ: 81, games: 2, wins: 1 },
    { mode: 'mayhem', champ: 238, games: 2, wins: 0 },
  ],
  patterns: [
    {
      mode: 'rift',
      kind: 'early_deaths',
      champ: null,
      title: 'Early deaths',
      evidence: 'In 6 of 8 losses you died 2+ times before 14:00 (1 of 6 wins)',
      focus: 'Your losses tend to start with early deaths; reaching 14:00 with 0–1 deaths is common in your wins.',
      bars: [
        ['Losses', 75],
        ['Wins', 16.7],
      ],
      unit: '%',
      games: 14,
      score: 0.41,
    },
    {
      mode: 'rift',
      kind: 'first_dragon',
      champ: null,
      title: 'First dragon',
      evidence: 'Enemy took first dragon in 7 of 8 losses (2 of 6 wins)',
      focus: 'First dragon goes to the enemy in most losses; lane priority before it spawns decides who contests it.',
      bars: [
        ['Losses', 87.5],
        ['Wins', 33.3],
      ],
      unit: '%',
      games: 14,
      score: 0.33,
    },
    {
      mode: 'rift',
      kind: 'cs',
      champ: null,
      title: 'Farm',
      evidence: 'CS/min 5.9 in losses vs 7.2 in wins',
      focus: 'Farm drops in losses: partly from being behind, partly CS left on the map.',
      bars: [
        ['Losses', 5.9],
        ['Wins', 7.2],
      ],
      unit: '/min',
      games: 14,
      score: 0.27,
    },
    {
      mode: 'mayhem',
      kind: 'deaths',
      champ: null,
      title: 'Deaths',
      evidence: '8.4 deaths per 10 min in losses vs 5.1 in wins',
      focus: 'Your death rate climbs in losses; staying alive tracks winning for you.',
      bars: [
        ['Losses', 8.4],
        ['Wins', 5.1],
      ],
      unit: '/10 min',
      games: 8,
      score: 0.22,
    },
    {
      mode: 'rift',
      kind: 'champion',
      champ: 103,
      title: 'Ahri results',
      evidence: 'Ahri: 3 wins in 9 games (33%) vs 60% on your other champions',
      focus: 'Ahri trails your other picks in this mode so far.',
      bars: [
        ['Ahri', 33.3],
        ['Others', 60],
      ],
      unit: '%',
      games: 9,
      score: 0.18,
    },
  ],
};

/** A new account: a few games, nothing to compare yet. */
export const macroFewGames: MacroReport = {
  games: 4,
  with_timeline: 4,
  modes: [
    { mode: 'rift', games: 1, wins: 0 },
    { mode: 'mayhem', games: 3, wins: 2 },
  ],
  champs: [
    { mode: 'mayhem', champ: 99, games: 2, wins: 2 },
    { mode: 'mayhem', champ: 103, games: 1, wins: 0 },
    { mode: 'rift', champ: 134, games: 1, wins: 0 },
  ],
  patterns: [],
};
