# Game-plan card

Date: 2026-11-10. Status: draft for review.

## Goal

At the start of a game, one card answers "how does this game go": which team is
stronger early and which late, who the scaling champions are on each side, what each
comp has and lacks, and the one line about my own lane. It is shown large while
loading and for the first two minutes, then folds to a single line that stays.

Stopping condition: the card shows for a real Rift game and a real ARAM or Mayhem
game with numbers that match the lolalytics pages by hand, and the parser tests pass
on the saved build pages.

## Scope

- Rift, ARAM, Mayhem (Mayhem uses lolalytics ARAM data, as the item advice does; the
  card says "ARAM data").
- Static data available before the game, and the draft both teams can see: within
  Riot's approved examples. Facts and comparisons, no orders ("their edge is after
  35 min", never "end before 30").

## Facts (checked 2026-11-10)

- The lolalytics build page carries two histograms, `time` (games) and `timeWin`
  (wins), over seven game-length buckets. `parse_build_page` already reads `time`
  for `avg_minutes` (bucket midpoints 12.5, 17.5, 22.5, 27.5, 32.5, 37.5, 44.0, so
  the buckets are under 15, 15–20, 20–25, 25–30, 30–35, 35–40, 40 and over).
  Ahri mid in `fixtures/data/lolalytics-build-ahri-middle.html`, decoded from the
  Qwik state, games / wins per bucket: 177 / 78, 1,247 / 624, 2,612 / 1,321,
  5,578 / 3,027, 4,464 / 2,340, 1,871 / 968, 690 / 355. So 44.1 % under 15 min,
  54.3 % at 25–30 and 51.4 % at 40 and over.
- The JSON endpoints (`counter`, `build-team`, `build-itemset`, `build-earlyset`) do
  not carry them: only build pages and matchup pages do.
- The build page is fetched today for my champion only (`champ_stats`). The draft
  fetches `counter` for all ten (`lolalytics_team::lane_stats`).
- `DraftReport` already has, per side: damage shares, frontline, engage, CC, peel
  and gap notes. `MatchupPlan` and `MatchupMemory` have my lane and my record.

## Design

### Data (`crates/data`, `crates/model-static`)

- `PageStats` and `ChampStats` gain `by_length: Vec<LengthWin>` (`#[serde(default)]`),
  `LengthWin { from_min: u32, games: u32, wins: u32 }`, parsed next to `avg_minutes`.
- `lolalytics::length_stats(http, champ, lane, mode, tier, region, patch)`: fetches
  only the build page (same URL as `champ_stats`, so the disk cache is shared) and
  returns `by_length`. Nine extra pages per game at most, cached with the usual TTL,
  fetched in the background through `state::ensure` like the portraits and ults.
  Missing pages leave that champion out of the curve.

### Decision (`crates/coach/src/plan.rs`, pure)

```text
plan::game_plan(input: &PlanInput) -> Option<GamePlan>
PlanInput { allies: &[(ChampKey, Option<Lane>)], enemies: &[...], length: &HashMap<ChampKey, Vec<LengthWin>>,
            draft: Option<&DraftReport>, matchup: Option<&MatchupPlan>, memory: &[MatchupMemory],
            notes: &LaneNotes, sd: &StaticData, mode: Mode }
GamePlan { phases: Vec<PlanPhase>, scalers: Vec<Scaler>, lines: Vec<PlanLine>, source: String, partial: bool }
PlanPhase { name: String, from_min: u32, to_min: Option<u32>, edge: f32 }   // win-rate points, mine − theirs
Scaler { champ: ChampKey, ally: bool, late: f32, early: f32 }
PlanLine { kind: String, text: String }       // "scaling", "comp", "spikes", "lane", "record"
```

- **Per champion**: win rate per bucket minus the champion's overall win rate from
  the same histograms, shrunk toward 0 by sample size (`common::shrink`). Buckets
  under `PLAN_MIN_GAMES` (200) are merged into their neighbour.
- **Phases**: three, each a games-weighted mean of buckets. Rift: early under 25,
  mid 25–35, late 35 and over. ARAM and Mayhem: under 15, 15–20, 20 and over
  (boundaries in `consts.rs`).
- **Team edge**: mean of my team's deltas minus the mean of theirs, per phase. Under
  `PLAN_EVEN` (0.7 points) the phase reads "even".
- **Scalers**: per side, the champion with the largest late delta and the one with
  the largest early delta, when above `PLAN_SCALER_MIN` (1.5 points).
- **Lines**, at most five, in this order:
  1. Scaling: "They are stronger late (+2.1 after 35 min: Kayle, Kassadin). You are
     stronger early (+1.4 before 25)." Or "No clear edge by game length."
  2. Comp, from `DraftSide` numbers and gap notes, as a comparison: "Engage: you 2,
     them 0. Frontline: you 1, them 3. Their damage is 80 % physical."
  3. Spikes: enemies whose `ChampNote.level6` is set (`champ_notes.toml`): "Level 6
     spikes on their side: Malphite, Amumu."
  4. My lane (Rift): the matchup win rate and the note's `wins` line.
  5. My record against these enemies, when `MatchupMemory` has any.
- `partial` is true when any champion's curve is missing; the card says "8 of 10
  champions".

What the numbers mean is stated on the card: win rate by game length is how a
champion does in games that end in that window. Early windows are full of stomps, so
only the difference between champions is used, never the raw rate.

### When it is built (`src-tauri/src/orchestrator.rs`)

- **Champ select** (Rift, all ten champions shown): from the draft's lineups, as a
  preview under the draft panel.
- **In game**: from the first Live API snapshot's `players` (this also covers ARAM,
  where enemies are hidden until the game loads). Lanes from `position`; the draft
  report is rebuilt from the same roster.
- Kept in `Memo` for the game; recomputed only when a background fetch lands.
- `AppState.plan: Option<GamePlan>` (not part of `GameAdvice`, so it survives the
  first seconds when advice is still empty).

### UI

- `web/src/components/GamePlan.svelte`: a three-segment bar (early / mid / late,
  coloured by edge with the points written on it), the scaler portraits under their
  phase, then the lines.
- Loading phase and game time under `PLAN_SHOW_S` (120 s): full card at the top of
  the in-game dashboard. After that: one line ("Stronger early · they scale: Kayle")
  that expands on click.
- Overlay: nothing in v1.
- Not spoken.

## Errors and gaps

- lolalytics down: no card (the champ-select preview and the in-game card both hide).
- An off-meta lane with few games: the champion's main-lane page is used and the
  card marks the champion with its data lane.
- Mayhem: augments decide a lot that ARAM curves don't know; the card keeps the
  scaling line and drops to "ARAM data" wording.
- Remakes and games already past 2:00 when the app starts: only the folded line.

## Testing

- `parse_build_page` on the Ahri mid and Ahri ARAM fixtures: seven buckets, the
  counts above.
- `plan::game_plan`: a late-scaling enemy team gives a negative late edge and names
  the scaler; all-even gives "No clear edge"; a missing curve sets `partial`; small
  buckets merge; ARAM boundaries are used for ARAM.
- Lines: comp line from a `DraftReport` fixture; no lane line outside Rift; no
  record line without memory.
- Smoke example prints the plan for one real ten-champion roster.
- Mock fixtures for champ select and in game gain `plan`; checked in the browser.

## Implementation status (2026-11-10)

Built:

- `LengthWin` and `by_length` on `PageStats` / `ChampStats`, parsed in `parse_build_page` from
  the same `time` / `timeWin` object as `avg_minutes`. Tests on the saved Ahri mid page (the seven
  counts above) and the Ahri ARAM page (96/63, 213/128, 141/64, 45/23, 5/2, 2/2, 1/0; sums to
  the header's 503 games).
- `lolalytics::length_stats`: build page only, same URL as `champ_stats` (shared disk cache);
  fetched per champion through `state::ensure` into `Shared::lengths`.
- `crates/coach/src/plan.rs`: `game_plan`, `roster_draft`; constants `PLAN_*` in `consts.rs`;
  tests in `crates/coach/tests/plan.rs`.
- Orchestrator: champ select preview (Rift, 5 + 5 in the draft report, lanes as the draft placed
  them), kept through loading, rebuilt in game from the Live API roster; `Memo.plan` rebuilt only
  when its inputs change (lineups, curves ready, lane win rate, record rows). `AppState.plan`.
- `GamePlan.svelte`: three-segment bar with points, scaler portraits under early / late, the
  lines, the note and source; full card in champ select (under the draft panel), while loading and
  before `show_s` (120 s), then the folded line that expands on click. Mock fixtures
  `champselect rift`, `loading`, `ingame start` (full) and `ingame rift` (folded) checked in the
  browser.
- Smoke example prints the plan for the live fixture's ten champions.

Deviations:

- `PlanInput.length` holds a `Curve = (Option<Lane>, Vec<LengthWin>)`: the lane the page is for,
  so the card can mark a champion whose data is its main lane's. `length_stats` returns the same
  pair; it uses the last 30 days when the patch has under `MIN_GAMES` games, and the main-lane page
  (no `lane`, lane None) when the lane has under `LANE_MIN_GAMES` (1,000). The main-lane page does
  not name its lane, so the card marks such a champion with `*` and "main-lane data" in its tooltip
  rather than the lane's name.
- `GamePlan` also carries `fold` (the folded line, built in `coach`), `curves` / `champs` (for
  "9 of 10 champions") and `show_s`; `PlanPhase` carries `even`; `Scaler` carries `off_lane`.
- Merging: a bucket under `PLAN_MIN_GAMES` merges into its smaller neighbour, smallest first; a
  merged bucket counts in the phase of its biggest part. Shrinkage `PLAN_SHRINK_K` = 400 games.
- A histogram bucket missing from the page counts as 0 games.
- In game the comp line comes from `roster_draft` (both teams' comp from the champions alone, no
  lane or synergy data), and the lane line from the laning panel's cards (no matchup page).
- Mayhem keeps every line; the scaling line starts "ARAM data, augments not included." and the
  source reads "lolalytics ARAM data".
- The in-game and loading cards are placed in `App.svelte` above the dashboard instead of inside
  `InGame.svelte`. A rebuild that yields no card (no curve fetched yet) keeps the previous one.

Left for real games:

- Rift: the champ select preview appears once all ten champions are in, its edge and scalers match
  the ten build pages by hand (win rate per bucket minus the champion's own), and the card carries
  through the loading screen into the game.
- ARAM and Mayhem: the card appears once the game loads (enemies hidden in champ select), with
  the ARAM boundaries (15 / 20 min) and, in Mayhem, the "ARAM data" wording.
- The full card folds at 2:00; starting the app after 2:00 shows only the folded line.
- An off-meta pick (e.g. a support Wukong) shows the main-lane mark; nine extra pages per game
  arrive within a few seconds and `partial` clears once they do.
