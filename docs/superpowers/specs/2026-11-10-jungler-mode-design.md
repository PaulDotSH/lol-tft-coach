# Jungler mode

Date: 2026-11-10. Status: draft for review.

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
