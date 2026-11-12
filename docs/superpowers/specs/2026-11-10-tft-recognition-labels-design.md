# TFT unit recognition, round two: real labels

Date: 2026-11-10. Status: approved; track A signals 1/3/4, track B collection and autolabel v2, and the head loader built (see Implementation status). Follows
`2026-10-25-tft-render-recognition-design.md`, whose approaches 1 and 2 failed the gate.

## Goal

Name enemy units in combat and on scouted boards well enough for the contested-units
and threat features: the same gate as before, on combat-start enemy crops **at least
60 % named and at most 5 % named wrong**.

Stopping condition, either way:

- the gate is met on leave-one-game-out over at least six collected games, and the
  head ships; or
- after 15 collected games the learning curve has flattened below the gate. Then
  enemy naming stays off, the collection code stays (it costs nothing), and the
  contested feature keeps using scouted boards and my own board only. This outcome
  is written into the spec's "Measured" section, not left open.

## What round one showed

- Renders alone: combat top-1 0.36, named right 0.12 at ≤ 5 % wrong.
- Real crops alone (about 70 per fold): 0.37 / 0.12.
- Renders + real crops: **0.53 / 0.24**. Each alone is weak; together top-1 rose by
  half with only two games of real labels in each fold.
- The blocker was labels, not the model: hand labels don't scale, and automatic
  labels from the client's match history failed because a fight could not be tied to
  its player (name plates read in 18 of 81 fights; when read, the label was right
  10 of 12 times).
- Render faults that remain: bind pose, missing shader colours, unknown per-unit scale.

So the plan is three tracks, in order of expected gain: (A) tie every fight to a
player, (B) collect and label real crops automatically every game, (C) decode with
what is known about that player. A fourth, (D) better renders, runs only if A–C
stall.

## Track A: which player am I fighting

Everything else depends on this. The plate reader already matches a plate's text to
the player list with edit distance (`plate_name`) and re-reads the far half of the
board in a fight (`viewed`); with that in place only 18 of 81 fights had a plate. So
the plate alone is not enough, and the link becomes a fusion of signals that each
shrink the candidate set of seven:

1. **Plate at any moment of the fight.** The opponent doesn't change during a fight,
   but their tactician moves and is often cut off by the top edge or covered at the
   first still read. Keep the fight's opponent from any read in the fight where
   exactly one other plate shows, not only the read the crops come from. First step:
   count, on the saved captures, in how many of the 81 fights a plate shows in at
   least one frame.
2. **The player list.** Spike on the saved combat frames: does the right-hand player
   list mark my current opponent (highlight, icon, order)? If it does, read that row.
3. **HP changes.** The reader already runs OCR over the player list's HP column but
   keeps only my own row (the tallest box). Keep every row, paired with its name:
   `TftSnapshot.lobby: Vec<(String, u32)>`. After a fight I
   won, my opponent is one of the players whose HP dropped that round, and the drop
   depends on the stage and on my surviving units (known from my bars at the end).
   After a fight I lost, my opponent is one of those whose HP didn't drop. Either
   way the candidates shrink, usually to one to three.
4. **Elimination.** Dead players are out; a fight against a ghost army is skipped
   (its board belongs to no living player's final board).
5. **Best explanation among the candidates left** (offline, in the labeller): the
   player whose final board explains the fight's crops best, accepted only with a
   margin over the runner-up. Round one tried this over all alive players and got 11
   of 19 right; over one to three candidates it is a much easier choice.

Output: `Fight { round, candidates: Vec<PlayerIdx>, opponent: Option<PlayerIdx> }`
written with the crops (track B); `opponent` is set live only by signals 1 and 2.
Gate for the track: on the three hand-labelled games, at least 80 % of fights in the
labelled rounds linked, at most 2 % linked wrong (hand-check against the labelled
units).

## Track B: collect and label every game

### Collect (`src-tauri/src/tft.rs`, `tft-vision`)

A setting "Collect TFT unit crops" (off by default; on in debug mode). When on, at
each fight's first still read and each scouted board read, the reader thread writes
to `%LOCALAPPDATA%\lol-coach\dev\crops\<match>\`:

- the bar-relative crop PNG (`units::crop`, the same code as naming);
- its embedding (`.f32`, so labelling and training never need the model);
- one JSON line: round, group (`combat` | `scout` | `mine`), bar position, star,
  the linked player and confidence, and for `mine` the confirmed unit.

The T1 tooling (`TFT_CROPS` in `board_replay` / `scout_replay`) already writes this
shape for replays; the app-side writer reuses it. Cost per fight: a few PNGs after
the read is done, off the reader's hot path. Nothing is written while a fight is
moving (first still read only), as today.

### Label (`tools/tft-render/autolabel.py`, second version)

After the game, the client's TFT match history gives every player's final board
(units, stars, items) and their last round. For each fight linked to a player:

1. Keep it only if the player's final board is a fair description of that round:
   the fight is within `LABEL_ROUNDS` (start at 4) of that player's last round.
   Earlier fights are left unlabelled.
2. Candidates are that player's final units (plus their summons). Score every crop
   against every candidate with the current head, then solve the assignment
   (Hungarian) with a "none of these" option per crop, constrained by stars from the
   HP bars (a ★3 crop can only take a ★3 unit).
3. Keep an assigned label only when its score beats the crop's second choice by
   `LABEL_MARGIN`. The rest stay unlabelled.
4. `mine` crops are labelled by the app's own confirmation (purchases and panel), as
   now, and are the only labels trusted without a margin.

Labels are weak: every training run reports how many auto labels a fold used and,
on the three hand-labelled games, how often auto labels agree with the hand labels.
Gate for the track: agreement at least 90 % on labels kept (round one's unlinked
version had 11 of 19).

### Train (`tools/tft-render/head.py`)

- Same features and head as round one's best (MLP, 512 hidden, no alignment),
  trained on render composites + all labelled real crops, real crops weighted up so
  a few hundred of them aren't drowned by 54,000 composites.
- A reject class from empty hexes, PvE units and crops the labeller marked "none".
- Evaluation is leave-one-game-out over the hand-labelled games plus any later game
  the user hand-checks; the report is the round-one table with one more column, the
  number of real labelled crops, so the learning curve is visible game by game.
- Self-training is allowed once (relabel with the new head, retrain) and no more:
  repeated rounds drift.

### Ship (`crates/tft-coach`)

`fixtures/tft/head.bin` (weights as f16, about 300 KB), loaded with `include_bytes!`
beside the seed. `Library::classify` gains a head path: one matrix product after the
embedding. The naming thresholds come from the evaluation run that produced the
file and are stored in its header, so they can't drift apart.

## Track C: decode with what is known

Per-crop naming ignores that a player's board barely changes between rounds.

- `Scout` keeps, per linked player, a belief per unit (decayed sum of head scores
  over that player's fights and scouted reads). A unit is named on a crop when the
  crop's score and the player's belief together pass the threshold: one weak look
  backed by three earlier rounds counts, a one-off look doesn't.
- Candidates exclude units whose pool is exhausted by known holdings (`scout::pool`),
  and on scouted boards are limited by that player's trait panel, as designed in
  round one.
- The gate is measured with decoding on, since that is what ships; the per-crop
  numbers are reported next to it.

## Track D: renders in the idle pose (only if needed)

Round one's renders are in the bind pose because Community Dragon's converted
folders have no skeletons or animations. The installed game's own files do
(`.skn`, `.skl`, `.anm` in the WAD archives on disk; reading installed files is not
memory access). Spike: convert one unit to glTF with its idle animation using an
existing converter, render it through `render.py`, and rerun `calibrate.py` on the
31 labelled units. Continue only if calibration top-1 rises from 0.37–0.45 to 0.6 or
more; otherwise drop the track.

## Order of work

1. Track A signals 1 and 3 measured on the existing 81 fights (no new captures
   needed). Cheap, and it decides how much of B is possible.
2. Track A signal 2 spike, if the link rate is still under the 80 % gate.
3. Collection setting and writer. From here every game the user plays adds data.
4. Autolabel v2, measured against the hand labels.
5. Retrain and report after each two new games; decoding (track C) once top-1
   passes 0.6.
6. Ship `head.bin` when the gate is met; otherwise track D, then the stop rule.

## Risks

- **Opponent link stays low.** Then labels are too few and the stop rule applies.
  This is why it is measured first.
- **Final boards differ from mid-game boards.** Handled by `LABEL_ROUNDS` and the
  margin; the cost is fewer labels from early stages, so early-stage units may stay
  weak. The report splits accuracy by stage.
- **Weak labels teach wrong names.** The agreement gate on hand-labelled games and
  the single self-training round bound this.
- **A new set resets everything.** Collection and labelling are set-agnostic; the
  render fetch is per set (`tft18_*`). After a set change the head is retrained from
  renders plus the first games; until it passes the gate again, enemy naming is off.
- Disk: about 60 small PNGs and embeddings per game (under 2 MB); the folder is
  pruned to the newest 40 games.

## Testing

- Opponent link: a plate read late in a fight links the whole fight; two plates in
  one read link nothing; HP candidates after a win (one player dropped by the
  expected amount) and after a loss (only players who didn't drop); a ghost fight is
  skipped.
- Collection: `board_replay` with `TFT_CROPS` and the app writer produce the same
  files for the same captured frames.
- Autolabel: a synthetic fight whose crops are the final board's own looks labels
  every crop; a fight outside `LABEL_ROUNDS` labels none; star constraints hold.
- Head in Rust: the same input gives the same top-1 and score as `head.py` on 100
  saved embeddings (tolerance for f16).
- Decoding: a unit seen weakly in three rounds is named, one seen once is not; a
  unit with no copies left in the pool is never named.
- The gate report itself is the acceptance test and is pasted into "Measured".

## Measured (2026-11-10)

**The saved captures are gone.** Round one's full frames (`%LOCALAPPDATA%\lol-coach\debug\tft\<game>\`)
are no longer on disk (nor anywhere under the user profile); what remains is the eval set
(`dev\render\eval\<game>\`: each fight's first-read crops, embeddings, `crops.jsonl`), the hand
labels (`tools/tft-render/eval/labels.jsonl`) and the client match history. So:

- The eval set has **25 fights** over the three hand-labelled games (5 + 6 + 14), all in labelled
  rounds, not 81 (the round-one figure counted something no longer reproducible).
- **Signal 1** (plate at any moment of the fight) can't be counted: only the first read of each fight
  was kept. On that read a plate shows in **5 of 25** fights (round one's rate).
- **Signal 3** (HP rows) can't be measured: no player-list HP was saved. Built and unit-tested only.
- **Signal 2 spike** (does the player list mark my opponent?) can't be done: no saved combat frame
  shows the player list. Left open; needs one debug game with frames.
- **Signals 1 (first read) + 4 (alive at that round, from history `last_round`) + 5 (best
  explanation, BOARD_MARGIN 0.5, renders-only MLP head)**, `autolabel.py` v2 on the three games:

| game | fights | linked | plate | one alive candidate | best fit | checkable | right | wrong |
|---|---|---|---|---|---|---|---|---|
| 20261025-024840 | 5 | 3 | 1 | 0 | 2 | 4 | 2 | 0 |
| 20261025-031617 | 6 | 4 | 1 | 0 | 3 | 3 | 2 | 0 |
| 20261025-153256 | 14 | 6 | 3 | 1 | 2 | 12 | 6 | 0 |
| **all** | **25** | **13 (52 %)** | 5 | 1 | 7 | 19 | **10** | **0** |

  "Checkable": the hand labels name one alive player whose final board holds most of the fight's
  labelled units (at least half). Every checkable link was right (0 of 10 wrong, 0 %); 3 linked
  fights weren't checkable. Elimination alone leaves 6.4 other players alive per fight on average,
  so it shrinks the set little before stage 5. **Track A gate (≥ 80 % linked, ≤ 2 % wrong): wrong
  rate met, link rate missed (52 %)**, before signals 1-at-any-moment, 2 and 3, which need a new game.
- **Autolabel v2 labels** (LABEL_ROUNDS 4, LABEL_MARGIN 1.0, Hungarian with "none", star constraint)
  with the renders-only head: 10 combat labels kept of 162 crops, 2 agreeing with the hand labels and
  5 not (3 on unlabelled crops). **Track B label gate (≥ 90 % agreement) missed**; the links are right
  but the renders-only head ranks a board's units badly (top-1 0.36). The labeller is meant to run
  with the renders + real crops head; that training run is out of this step.
- Match history names game folders by local time: on 2026-10-25 (DST change) `20261025-024840`
  matched its game only an hour off, which round one's `autolabel.py` missed; v2 tries ±1 h.

## Out of scope

My own board's recognition, items, the HP-bar and star readers, streamer VODs, and
any use of the Riot API beyond what the client's local match history already gives.

## Implementation status (2026-11-10)

Built (branch `feat/tft-labels`):

- **Track A.** `TftSnapshot.lobby: Vec<(String, u32)>`: each player list name paired with the HP
  number on its row (`tft-vision`, `lobby`; centres within ROW_DY = 30 px). `tft_coach::fights`
  (`Tracker::fights()`): a roster by first sighting (`PlayerIdx`), `Fight { round, candidates, opponent }`;
  the opponent from any read of the fight with exactly one plate (all such reads agreeing); on the
  second read of the next round the HP rows (each confirmed by two agreeing reads, as my own HP)
  shrink the candidates: won → players whose HP dropped, narrowed to `expected_drop(stage, survivors)`
  when that leaves any; lost → players whose HP held; dead (0 HP at the start) are out; a plate the HP
  rows contradict is dropped; no candidate left with an odd number alive is a ghost fight, skipped.
  Tests in `crates/tft-coach/tests/fights.rs` and the vision unit tests.
- **Track B collection.** Setting `tft_crops` ("Collect TFT unit crops", off by default, always on in
  debug mode). The writer moved from `examples/common/crops.rs` to `tft_vision::crops`, shared by the
  replays (`TFT_CROPS`, `eval_crops`) and the app (`tft.rs` `CropWriter`, after each read's advice);
  lines gain `player`/`link`, fights go to `fights.jsonl`; the folder is pruned to the newest 40 games.
  Test: `the_app_writes_the_crops_the_replays_write` (same frames through `Crops::open` + `observe`, as
  `eval_crops`, and through the app writer: identical files), plus a prune test.
- **Autolabel v2** (`tools/tft-render/autolabel.py`) and its synthetic tests
  (`tools/tft-render/test_autolabel.py`: final-board crops label every crop; outside LABEL_ROUNDS none;
  star constraints; one slot per crop; links; head export).
- **Head in Rust** (`tft_coach::head`, `Library::with_head` / `classify_head` / `head_name`): TFH1
  format with the naming thresholds in its header; `fixtures/tft/head.bin` is an empty placeholder, so
  head naming stays off; when present it names a fight's red bars after learnt looks and before the
  seed vote (`how` "^"). `head.py --export head.bin` writes the file and its parity cases;
  `--tiny-fixture` wrote `fixtures/tft/head_tiny.bin` + `head_tiny_parity.json` (114 cases), which the
  Rust parity test checks (top-1 equal, probability within 5e-3); the shipped head is checked the same
  way once it exists.

Deviations:

- No captures on disk: signals 1 and 3 and the signal 2 spike unmeasured (see Measured); 25 fights, not 81.
- `Library::classify` is unchanged; the head path is separate (`classify_head`, `head_name`), since
  its probabilities and the looks' similarities have different thresholds.
- `Fight` lives in `tft-coach` (not `tft-model`): it isn't sent to the UI.
- App game folders are named by the reader's start in unix seconds (no match id is known live);
  `autolabel.py` matches both name styles to the history by time.
- The "missing-file guard" is an empty `head.bin` (as `looks_seed.bin`), not a missing file.
- Player damage per stage (`fights::STAGE_DAMAGE`, `UNIT_DAMAGE`) are unmeasured Set 18 guesses, used
  only when they leave a candidate.
- `board_replay` still writes only `mine` (it shares a folder with `scout_replay`); `eval_crops` is the
  replay that writes every group through the same `observe` as the app.

Open:

1. Play one game in debug mode (crops on) with frame dumps: measure signal 1 at any moment, signal 3
   (HP rows read, candidates per fight) and do the signal 2 spike; check ROW_DY and the damage table.
2. Training runs (renders + real crops head, reject class), autolabel with that head, the gate report
   with the real-crop column, `head.py --export` once the gate is met.
3. Track C decoding, track D renders, collecting new games.
