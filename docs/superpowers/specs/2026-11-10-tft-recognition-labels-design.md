# TFT unit recognition, round two: real labels

Date: 2026-11-10. Status: draft for review. Follows
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

## Out of scope

My own board's recognition, items, the HP-bar and star readers, streamer VODs, and
any use of the Riot API beyond what the client's local match history already gives.
