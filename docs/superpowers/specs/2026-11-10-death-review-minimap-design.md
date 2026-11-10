# Death review: what the minimap showed

Date: 2026-11-10. Status: draft for review.

## Goal

Each death in the review says whether it could be seen coming: which enemies were on
the minimap in the seconds before, whether a gank warning had fired, or whether the
killer came out of fog. Across the game it becomes a pattern line: "4 of 6 deaths:
the killer was on the minimap 5 s or more before" or "3 deaths to enemies never seen:
river and enemy jungle, no ward".

Stopping condition: the lines appear in the end-of-game recap and in the match review
for a real Rift game watched live, and replaying a saved debug session through the
tests gives the same lines.

## Scope

- Rift games the app watched live with gank warnings on (the minimap thread only
  runs then, and only while the game is in the foreground). Other games: no change.
- Facts about what was on my own screen. Nothing is shown during the game: this is
  review only, so it adds no live information.

## Facts (checked 2026-11-10)

- `orchestrator::spawn_minimap` reads enemy blips about once per scan, names them by
  portrait, feeds `GankTracker::observe` with world coordinates and gets the current
  gank `Call`s. It publishes only the calls (`Shared::gank_calls`).
- `GankTracker` keeps a short history of `(time, blips)` and drops it after
  `GANK_MAX_DT`.
- `selfcoach::GameLog::observe` builds a `DeathNote` per death from the Live API
  tick. It knows the killer and assisters, not positions.
- The match review (`analysis::review`) takes deaths from the timeline, with their
  position (`KillEvent.pos`), and merges the live `DeathNote` within `SAME_DEATH` (3 s).
- Only enemy blips are read. My own position is not read live; after the game the
  timeline gives where I died.

## Design

### 1. Keep the sightings (`coach::gank`, `src-tauri`)

- New pure type in `coach`:

  ```text
  Sighting { time: f32, x: i32, y: i32, champ: Option<ChampKey> }   // world coords from blue side
  SightLog { reads: VecDeque<(f32, Vec<Sighting>)>, calls: Vec<(f32, String)> }
  SightLog::push(time, &[(i32, i32, Option<ChampKey>)], blue, &[Call])
  SightLog::window(from, to) -> impl Iterator<Item = &Sighting>
  SightLog::covered(from, to) -> f32      // share of the window with reads
  ```

  It keeps the last `SIGHT_KEEP_S` (30 s) of reads and the time and key of every
  gank call's first appearance.
- The minimap thread owns one and mirrors it into `Shared::sights` (a mutex, like
  `gank_calls`). It resets with the tracker when the game changes.
- Names use the tracker's result after its teleport check, so a rejected name stays
  an unnamed blip.

### 2. Stamp each death (`coach::selfcoach`)

`GameLog::observe` gains `sights: Option<&SightLog>`. When it records a death at
time `t` it fills new `DeathNote` fields, all `#[serde(default)]`:

```text
DeathNote.sight: Option<DeathSight>
DeathSight {
  covered: f32,                    // share of [t − 15, t] the minimap was read
  seen: Vec<SeenBefore>,           // killer and assisters first, then others
  warned: Option<(f32, String)>,   // seconds before death, the call's text
}
SeenBefore { champ: Option<ChampKey>, first: f32, last: f32, x: i32, y: i32 }
           // seconds before the death first and last seen; where last seen
```

Unnamed blips are kept as `champ: None`. `covered < SIGHT_MIN_COVER` (0.5): `sight`
is None (alt-tabbed, thread off, capture failed).

### 3. Classify (`coach::sight`, pure)

`sight::verdict(note: &DeathNote, lane_opponents: &[ChampKey]) -> Option<SightVerdict>`

| Verdict | Rule | Line |
|---|---|---|
| `Warned` | a gank call fired 2 s or more before the death | "Warned 6 s before: Lee Sin from the river" |
| `Visible` | killer or an assister, not my lane opponent, first seen ≥ `SEEN_EARLY_S` (4 s) before, no call | "Lee Sin was on the minimap for 7 s before" |
| `Lane` | everyone involved is my lane opponent(s) | no line (a lost trade, not a gank) |
| `Fog` | none of the killer and assisters was seen in the 15 s before, and no unnamed blip within reach | "Lee Sin came out of fog" |
| `Unclear` | only unnamed blips, or seen under 4 s before | "An enemy showed 2 s before" |

Constants in `consts.rs`. Executions (no killer) get no verdict.

After the game, when the death has a position (`DeathNote.pos` from the timeline):
`Visible` and `Unclear` are kept only if the last sighting was within
`SEEN_NEAR` (4,500 units) of where I died; a blip across the map says nothing about
this death. `Fog` gains the place: "came out of fog in the river".

### 4. Notes across the game (`selfcoach::review_deaths`)

Added to `DeathReview.notes`, each needing at least two deaths:

- "N of M deaths: the killer was on the minimap 4 s or more before" (Warned + Visible).
- "N deaths came out of fog in <places>" (Fog; places from the review). When my
  vision score per minute is under the benchmark, the existing vision takeaway is
  placed next to it rather than repeated.
- "N warnings were followed by a death within 10 s" (Warned).

The first note maps to the review's `Focus` when it covers half the deaths or more:
"Look at the minimap before stepping up: 4 of 6 deaths were visible first." No new
goal kind in v1; a `GoalKind::SeenDeaths` can follow once the numbers have been seen
over a few weeks.

### 5. UI

- `DeathReview.svelte`: one extra line per death with a small icon (eye, warning,
  fog). `DeathMap.svelte`: for the selected death, dots for `seen` positions with
  their "seconds before" label, and a line from the last sighting to the death.
- Recap right after the game: the per-death lines without positions; the match
  review adds the distance filter and the map once the timeline is available.

## Errors and gaps

- A killer who was visible but whose portrait wasn't recognised counts as an unnamed
  blip: `Unclear`, never `Fog`. The Fog rule is the strict one because "came out of
  fog" is the claim most likely to be wrong.
- Blips are champions with a red ring; wards, clones and some summons may be missed
  or misread, as in the gank warnings.
- A minimap covered by the scoreboard or the shop lowers `covered` and can hide a
  sighting; under 0.5 there is no verdict.
- Recaps saved before this change have no `sight`: nothing shown.

## Testing

- `SightLog`: window bounds, pruning, coverage with gaps, reset when time goes back.
- `sight::verdict`: one test per row of the table, plus the distance filter with and
  without `pos`, and an execution.
- Replay: `crates/coach/tests/gank_replay.rs` already replays real games' blip reads
  (`fixtures/coach/gank_*.json`, time + blips). Those fixtures hold no deaths, so one
  new fixture is recorded in debug mode with both the `minimap-blips` dump and the
  Live API snapshots of the same game; the test feeds a `SightLog` and a `GameLog`
  and asserts the verdict of each death (checked by hand once against the saved
  minimap frames).
- `review_deaths` notes: thresholds, wording, the two-death minimum.
- Old recap JSON without `sight` still deserialises (`#[serde(default)]`).
- Mock: `web/src/mock/analysis.json` gains two deaths with `sight`; checked in the
  browser mock.
