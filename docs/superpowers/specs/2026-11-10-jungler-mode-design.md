# Jungler mode

Date: 2026-11-10. Status: phase 1 built; phase 2 stopped at the spike gate (see Implementation status).

## Goal

When I play jungle, the coach helps with a jungler's questions instead of a laner's:
am I clearing on pace, which lanes are open to a gank right now, where the enemy
jungler was last seen and what that leaves free, and, after the game, how my clears,
ganks and objective presence went.

Stopping condition for phase 1: in a real jungle game the pace line, the lane board
and the jungle review section show, the laner-only tips are gone, and the replay
tests pass. Phase 2 (camp timers) has its own gate below.

## Scope

- Rift, when I am the jungler: `players[me].smite`, or `position == Jungle` (the same
  test `gank_enemies` uses for the enemy jungler).
- Everything shown is on my own screen or static: enemy laners' positions when they
  are on the minimap, levels, items, summoner spell names, announced objectives.
  Never enemy camps, never the enemy jungler's position in fog (R3 in `ideas.md`
  stays rejected).
- Options with reasons, not orders: the lane board ranks all three lanes and says
  why; it never says "gank top now" (R11).

## Facts (checked 2026-11-10)

- The Live API gives me my CS (lane and jungle together), level, items and events.
  It has no positions and no camp events.
- The minimap thread reads enemy blips, named by portrait, in world coordinates.
  `GankTracker` already treats the jungler differently in places: invade wording for
  `Lane::Jungle`, roam calls for every laner, where the enemy jungler was last seen
  (`JunglerSeen`), and a laner "at home" test (`at_home`).
- Match-history timelines give per-minute frames with `jungle_cs`, level, gold and
  position, and kill events with positions; `fights.rs` already knows where I was
  when each epic monster died (`Presence`).
- Item-gold differences per lane exist live (`GoldDiff` / `LaneDiff`).
- My own position is not read live. Allied blips are not read.

## Phase 1: from data the app already has

### 1. Clear pace (`coach::jungle::pace`, pure)

A quiet line in the tips row, early game only:

- Before 4:00: level against a small table in `consts.rs` (first guesses, like the
  objective timers): level 3 by `JG_L3_BY` (2:35), level 4 by `JG_L4_BY` (3:30, when
  the scuttle crabs spawn). "Level 4 at 3:22: on pace" / "Level 3 at 3:30: one camp
  behind".
- From 4:00 to 14:00: jungle CS per minute against my champion's lolalytics
  benchmark (`Benchmarks::cs_per_min` for the jungle lane already holds minions +
  jungle CS), the same wording as the laner's CS tip, which it replaces.
- The laner tips that don't apply are dropped for the jungler: lane CS pace, "lane
  opponent" matchup lines, the laning panel.

### 2. Lane board (`coach::jungle::lanes`, pure)

One row per lane (top, mid, bot), ranked, each with one or two reasons. Inputs and
what they add:

| Signal | Source | Reason text |
|---|---|---|
| Enemy laner past the middle of the lane toward our turret | minimap blip, a new `gank::pushed(lane, blip)` beside `at_home` | "Darius is pushed up" |
| Enemy laner not seen for a while | `GankTracker::named` last-seen time | "Sylas not seen for 20 s" (lowers the lane; it may be a roam) |
| Setup on my laner | `traits.cc` of the allied laner | "Maokai has CC" |
| Escape on theirs | a new `traits.mobile` flag, or hand list, of dashes and blinks | "Jinx has no dash" |
| Summoner spells | Live API spell names, no cooldowns. New: `Player.spells` (the parser keeps only Smite today) | "no Flash" only when they don't run it |
| Lane state by gold | `LaneDiff` | "lane is even" / "your laner is 900 behind: a kill is unlikely" |
| Level edge | visible levels | "your side is 6, theirs is 5" |
| Enemy jungler seen on the other side | `JunglerSeen` within `JG_SEEN_FRESH_S` | "Lee Sin was just seen bot side" |

- Score = weighted sum, weights in `consts.rs`. Rows under a floor show as "nothing
  special". Shown as a small three-row card in the place of the laning panel.
- Only the blip signals change second to second; the card is recomputed on the
  minimap thread's cadence through the same path the gank calls use, but it is never
  spoken and not on the overlay in phase 1.

### 3. Calls for the jungler (`coach::gank`, `coach::calls`)

- Kept: invade ("X invading your jungle"), roams of every laner, objective calls.
- Changed: the "jungler seen" call becomes "Lee Sin seen top side": for a jungler it
  means the other side's camps and objective are free of him. The wording stays a
  fact.
- New, quiet tip: "Dragon in 60 s" already exists as a call; for the jungler the tip
  adds whether I hold a full item's gold ("back first: 1,300 g") using the existing
  back planner numbers.

### 4. Jungle review (`coach::jungle::review`, pure, over `MatchDetail`)

A "Jungle" section in the match review, replacing the Laning section when I jungled:

- **First clear**: level and jungle CS at 3:00 and 4:00 from the frames ("Level 4,
  24 jungle CS at 4:00").
- **Clear rate**: jungle CS per minute for 0–5, 5–10, 10–15, against the benchmark.
- **Ganks**: kill events before 14:00 in a lane zone where I killed or assisted,
  per lane, with the result ("Top: 2 ganks, 1 kill, 0 deaths"). An attempt without a
  kill can't be seen in the timeline; the section says "ganks that got a kill".
- **Objectives**: for each epic monster, which team took it and my `Presence`
  (already computed by `fights.rs`), plus the enemy jungler's alive/dead state.
- **Counter-jungling**: minutes where my frame position is in the enemy jungle and
  my jungle CS rose by 4 or more since the previous frame.
- **Against their jungler**: gold, level and jungle CS difference at 5, 10 and 15
  (the existing `PhaseRow` against the enemy jungler as opponent).

`MatchAnalysis.jungle: Option<JungleReview>`, `#[serde(default)]`.

## Phase 2: my own position and camp timers (spike first)

Camp respawn timers for camps I cleared are information the client itself shows, so
they are allowed. They need to know which camp I just cleared, which needs my
position.

1. **Spike**: find my own marker on the minimap in the saved debug frames
   (`minimap` PNGs): the white camera rectangle (its centre is my position while the
   camera is locked) and my own icon's ring. Measure how often each is found and how
   far it is from the truth on frames where I am in a known spot (fountain at 0:00).
   Gate: position within 600 units on 90 % of frames, or phase 2 stops here.
2. `ocr::minimap::me(frame, sq) -> Option<(f32, f32)>`; the minimap thread passes it on.
3. `coach::jungle::Camps`: when my CS rises by a camp's worth while my position is
   within a camp's radius (camp coordinates in `consts.rs`), start that camp's
   respawn timer (static respawn times). Shown as a compact list sorted by time.
   Only camps I took; never the enemy's.
4. The same position makes the death review's distance check live (see the death
   review spec) and gives the lane board "distance to lane" as a signal.

## Model and UI

```text
GameAdvice.jungle: Option<JungleAdvice>            // Some only when I jungle
JungleAdvice { pace: Option<String>, lanes: Vec<LaneOpen>, camps: Vec<CampTimer> }
LaneOpen { lane: Lane, score: f32, reasons: Vec<String>, enemy: Vec<ChampKey> }
CampTimer { name: String, respawn_at: f32 }        // phase 2
```

- `web/src/components/ingame/Jungle.svelte`: pace line, the three-row lane board,
  later the camp list. `InGame.svelte` shows it instead of `Laning.svelte`.
- Review: `web/src/components/idle/JungleReview.svelte`.

## Errors and gaps

- Minimap not read (thread off, game not focused): the lane board falls back to the
  static signals only and says "no minimap".
- A laner out of sight isn't known to be pushed or roaming; the board says "not
  seen", it doesn't guess.
- Pace thresholds are first guesses and differ by champion; the benchmark part
  (from 4:00) is per champion, the level table is not.
- Swiftplay runs as `Mode::Rift` with a faster early game (the gank replay fixtures
  are Swiftplay games). The level table is wrong there: it is hidden for that queue
  until its own row has been measured; the rest stays on.

## Testing

- `jungle::pace`: on pace, behind, after 4:00 against a benchmark, no benchmark.
- `jungle::lanes`: each signal alone moves the right lane; a pushed laner with the
  enemy jungler on that side ranks below a pushed laner without; no blips gives the
  static ranking.
- `gank::pushed`: unit tests beside the `at_home` ones; the replay fixtures
  (`fixtures/coach/gank_*.json`) run with `lane = Jungle` and the calls are
  snapshotted.
- `jungle::review` on a jungle game's saved match + timeline (a new anonymized
  fixture next to `match_game_flex.json`): first clear, ganks per lane, objectives.
- Phase 2: `ocr::minimap::me` on saved frames with hand-marked positions; `Camps`
  with synthetic CS and position sequences.
- Mock fixture with `jungle` set; checked in the browser mock.

## Implementation status (2026-11-10)

### Built (phase 1)

- `coach::jungle` (`crates/coach/src/jungle.rs`, `jungle/lanes.rs`, `jungle/review.rs`); `GameAdvice.jungle`
  set by `live_extras` on the Rift when I jungle (`Player.smite` or `position == Jungle`).
- Pace: level table before 4:00 (`JG_L3_BY` 2:35, `JG_L4_BY` 3:30, "Level 4 at 3:22: on pace", "Level 3 at
  3:30: one camp behind"), hidden for Swiftplay; 4:00–14:00 CS against the benchmark ("CS 24 behind the average
  pace (5.0/min vs 7.4): take your camps between ganks", "CS on pace (…)"), nothing without a benchmark.
- Laner tips dropped for the jungler: the "cs" tip, the laning panel (`lane_matchup::laning` is empty).
- Lane board: `jungle::board` (static part: CC of my laner, "no dash", "no Flash", lane gold from `LaneDiff`,
  levels) and `jungle::lanes(board, Option<&MapView>)` (pushed, not seen, enemy jungler seen). `gank::pushed`
  beside `at_home`; `GankTracker::view` gives the minimap part. The orchestrator puts the board in `GankCtx`;
  the minimap thread recomputes it every read, stores it in `Shared::jungle_lanes` and pushes it to the UI with the
  gank calls. Never spoken, not on the overlay.
- `Player.spells` from the Live API parser (internal names, `SummonerFlash`); `GameSnapshot.swiftplay`.
- Calls: for the jungler the enemy-jungler call is "Lee Sin seen top side" / "Lee Sin seen top side.". Dragon
  tip ("jg-objective"): "Dragon in 1:10: back first: 5,000 g buys X" or "… no full item yet, 650 g short of X",
  from `GoldEta`, 0–90 s before the spawn.
- Review: `MatchAnalysis.jungle` (`JungleReview`), `laning` is None when it is set. First clear at 3:00/4:00,
  clear rate 0–5/5–10/10–15 (jungle CS/min and all CS/min against the benchmark), ganks that got a kill per
  lane, objectives with my presence and the enemy jungler dead or alive, minutes with camps in their jungle,
  `PhaseRow`s at 5/10/15 with jungle CS against their jungler.
- UI: `ingame/Jungle.svelte` instead of the laning panel, `idle/JungleReview.svelte` instead of `Laning`;
  mocks `ingame jungle` and a Diana jungle game in the mock history.
- Tests: `crates/coach/tests/jungle.rs` (pace, every lane-board signal, the jungler-side ranking, no blips,
  dragon tip, laner tips gone, review), `gank::pushed`/`view`/wording unit tests, the four gank replays as the
  jungler snapshotted in `fixtures/coach/gank_jungle_calls.txt` (`UPDATE_SNAPSHOTS=1` rewrites it), the Live API
  fixture's spells.

### Deviations

- Mobility is a hand list (`consts::NO_DASH`, Data Dragon ids of champions with no dash, blink or leap), not a
  `traits.mobile` flag; champions missing from it never get "no dash".
- `JungleAdvice.minimap: bool` added so the card can say "no minimap" (minimap thread off, gank warnings off,
  game not focused: the static board is shown).
- The pace line sits at the top of the Jungle card (`JungleAdvice.pace`), not in the tips row.
- The enemy jungler's side on the board comes from his last named sighting (`GankTracker::view`), the same data as
  `JunglerSeen`, which keeps the side of the first sighting of an appearance.
- Rows under the floor say "nothing special" even when the reasons are negative (enemy jungler there).
- Lane gold for bot sums both `LaneDiff` rows (ADC and support).
- Swiftplay is read from the Live API's `gameMode` (`SWIFTPLAY`); the snapshot has no queue id.
- Jungle review fixture: no saved match + timeline of a game I jungled exists on disk (history has one, Shen
  ranked solo 4017526952, without details). `fixtures/clients/match_game_jungle.json` is synthetic: the flex
  fixture with me as the blue Diana jungler (puuid and name moved), using `match_timeline_flex.json`.

### Phase 2 spike (saved debug minimap frames)

- Data: 781 minimap PNGs (one per 3 s) in 5 debug sessions; truth from the saved timelines of 3 Swiftplay games
  (Morgana support red 4025231045, Yasuo mid blue 4025243448, Zed mid blue 4025257442) and fountain frames at
  ≤ 3 s (incl. Thresh, no timeline): 42 checks, 35 timeline frames within 1.5 s of a PNG and 7 fountain frames.
- Camera rectangle: pure white (≥ 240) 2 px lines, 92 × 51 px on a 338 px map (16:9 client); found by long white
  runs, a clipped one completed with the median size. Found on 766 of 781 frames (98 %), on 37 of the 42 checks.
- Distance to the truth: within 600 units on 31 of 42 checks (74 %; 84 % of the frames where it was found),
  median 365. The view's centre sits north of the champion: median offset truth − camera (−74, −275). With that
  offset: 32 of 42 (76 %; leave-one-out 31 of 42), median 248. Misses: no rectangle on 4 of 5 fountain frames at
  ≤ 2.2 s and once at 14:00, 1.1–1.7k off three times in the support game (camera unlocked), 600–850 twice.
- Own icon: my ring is the allies' blue (81, 162, 229) and in lane mostly under the enemy laner's red ring; a
  blue-ring finder put a ring within 900 of the camera on 11 of 42 checks, 7 within 600. Not usable without
  portrait matching my own champion.
- Gate (600 units on 90 %) not met: phase 2 stops. `ocr::minimap::me` and `coach::jungle::Camps` are not built;
  `JungleAdvice.camps` stays empty (the card draws the list when it has rows).

### Left open (needs a real game)

- A real jungle review fixture: after a game where I jungle, start with `run.cmd debug` so the match-history
  probe saves `…games_<id>.json` and `…game-timelines_<id>.json`; anonymize next to `match_game_flex.json`, swap
  it in for the synthetic one, and check first clear, ganks per lane and objectives against the game.
- Pace thresholds: compare `JG_L3_BY`/`JG_L4_BY` with real full clears (normal Rift), and measure a Swiftplay row
  so the level table can come back there.
- Lane board in a live jungle game: pushed calls (`PUSH_BAND`/`PUSH_BASE`/`PUSH_PAST`), "not seen" times, the
  weights and floor, the board updating on the minimap cadence, "no minimap" with the game unfocused.
- Live API: confirm summoner spell names for upgraded Smite and Unleashed Teleport, and the "no Flash" line.
- Hear "X seen top side" and see the dragon back tip in a game.
- Phase 2: record frames (debug mode) of a jungle game with the camera locked and its timeline; try a portrait
  match of my own champion on the blue rings, with the camera centre plus offset as the prior, and re-run the gate.
