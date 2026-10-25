# Plan: TFT unit recognition from 3D renders

Spec: `docs/superpowers/specs/2026-10-25-tft-render-recognition-design.md` (approved 2026-10-25).
Goal: a render-seeded looks library built into the app, measured on real crops against the gate
(combat-start enemies ≥ 60 % named, ≤ 5 % wrong).

## Ground rules (every task)

- Never run rustfmt / cargo fmt; hand-formatted wide (~150 cols), terse `///` docs, constants at the
  top of files. Coordinates at 2560 width, scaled by width/2560.
- Commit locally after each task; no `Co-Authored-By` or other attribution; never push.
- **No GPU renders, release builds or long CPU jobs while a game runs.** Before a heavy step check
  `tasklist` for `League of Legends.exe` and `TFTClient-Win64-Shipping.exe`; if one runs, wait.
- Never click or focus windows. Blender only headless: `"C:\Program Files\Blender Foundation\Blender 5.2\blender.exe" -b -P <script> -- <args>`.
- Dev data lives outside the repo in `%LOCALAPPDATA%\lol-coach\dev\render\` (`assets/`, `renders/`,
  `eval/`, `bg/`). Only code, small label lists and the final seed go in the repo.
- Python: system Python 3.12 (`py -3.12 -I`), dependencies `requests pillow numpy` only.
- Parallel agents: own worktree `repos/apps/lol-coach-wt/<name>`, branch `feat/<name>`, and
  `CARGO_BUILD_BUILD_DIR=<worktree>/target-build CARGO_BUILD_JOBS=2`.

Inputs available: captures `%LOCALAPPDATA%\lol-coach\debug\tft\20261025-024840`, `…-031617`, plus the
Choncc game captured on 2026-10-25 (`…\debug\tft\<newest>`, has scouting views if the user clicked
other boards). DINOv2 model:
`C:\Users\TempAdmin\.cache\huggingface\hub\models--onnx-community--dinov2-small\snapshots\8b1f705a3a7f6f062f6bdd21986c1583d3ef105d\onnx\model.onnx`.
Player name `vandboabe`.

## Order

```
T1 eval set ─────────────┐
T2 assets ── T3 render.py ┴─ T5 calibrate ── T6 full render + seed ── T8 gate
T4 library TFL2 + seed ── T7 scout vote/constraints ─────────────────┘
```

T1, T2→T3 and T4→T7 can run in parallel (three agents). T5, T6, T8 run after, in order.

---

## T1. Evaluation set (Rust examples + labels)

Files: `crates/tft-vision/examples/board_replay.rs`, `scout_replay.rs`, new
`crates/tft-vision/examples/eval_crops.rs`, `tools/tft-render/eval/labels.jsonl` (small, in repo).

1. Add `TFT_CROPS=<dir>` to `board_replay` and `scout_replay`: for every piece/foe read used for naming,
   write the bar-relative crop (`units::crop`, as embedded) as PNG plus a JSON line
   `{file, frame, group: mine|scout|combat, x, y, star, bench, api|null, how}`; also the embedding
   (`.f32` next to the PNG) so evaluation needs no model run.
2. Groups:
   - `mine`: my pieces named Bought/Panel and confirmed (as learning uses).
   - `combat`: red-bar units from the first still read of each fight (all bars full).
   - `scout`: units on a viewed other board in quiet planning.
3. Label `combat` and `scout` crops by hand: contact sheets of 40 crops with the set's square icons
   (`tft18_<x>_square.png`) beside candidates; record `api` in `labels.jsonl` (keyed by capture
   file + bar position). Unsure → `"?"` (excluded). Aim ≥ 150 combat, all scout crops available.
4. If `RIOT_API_KEY` is set: `tools/tft-render/riot_boards.py` finds the captured matches by puuid
   and start time and prints every player's final units; use it to check last-round combat labels
   (report agreement).

Done when: `%LOCALAPPDATA%\lol-coach\dev\render\eval\` holds crops + embeddings for the three games,
`labels.jsonl` is committed, and counts per group are reported.

## T2. Assets (`tools/tft-render/fetch.py`)

1. Read `fixtures/tft/cdragon_set18.json`: Set 18 units (cost 1–5 and summons that get HP bars),
   `characterName` → folder `tft18_<name>` lowercase.
2. Parse Community Dragon directory listings
   (`https://raw.communitydragon.org/latest/game/assets/characters/<folder>/skins/base/` and its
   OBJ subfolder); download every `.obj` and `.png` into `assets/<folder>/`.
3. Write `assets/manifest.json`: per unit `{api, folder, objs: [{name, verts}], textures: [..]}`,
   and the list of units missing a mesh or texture.

Done when: the manifest covers every set unit or lists why not; re-running skips downloads.

## T3. Renderer (`tools/tft-render/render.py`, Blender)

1. Args: manifest, unit list, out dir, params JSON (`pitch`, `ortho_scale`, `yaws`, `sizes`, `lights`,
   `res` = 256).
2. Per unit and variant: import OBJs (`bpy.ops.wm.obj_import`), one material per OBJ with the matching
   texture (by name; fall back to the single `*_tx_cm.png`), unlit-ish Principled (low roughness
   detail). Variants: all meshes minus `VFX_*`; plus each mesh holding ≥ 30 % of vertices alone
   when there are several such groups (forms).
3. Orthographic camera at `pitch`, fixed `ortho_scale` for **all** units (keeps true relative sizes),
   origin at the unit's feet, facing yaw steps; `film_transparent`; EEVEE, GPU (HIP).
4. Output `renders/<api>/<variant>_<yaw>_<size>_<light>.png` (RGBA) and `meta.json` with the mesh
   top in pixels (for placing the HP bar).
5. Smoke test: 3 units (Ahri, Garen-like big unit, a summon) → look at the PNGs.

Done when: the smoke renders look right (silhouette, texture, transparency) and take < 0.5 s each.

## T4. Library: seed looks and TFL2 (`crates/tft-coach/src/library.rs`)

1. Looks carry `seed: bool`. `classify` returns `(api, sim, seed)` for the best match per unit
   (best real and best seed kept apart); callers updated.
2. `learn` never evicts or dedupes against seed looks; `to_bytes` writes only learnt looks.
3. Format `TFL2`: header, then per look name, ★, seed flag, dim, f16 values (`half` crate).
   `from_bytes` reads TFL1 and TFL2.
4. `Library::with_seed(bytes)`; the app (`src-tauri/src/tft.rs`) passes
   `include_bytes!("../../fixtures/tft/looks_seed.bin")` when the file is non-empty (commit an empty
   placeholder until T6).
5. Tests: seed survives learning and isn't saved; TFL1 still loads; f16 round-trip within 1e-3.

Done when: `cargo test --workspace` and clippy are clean; behaviour unchanged with an empty seed.

## T5. Calibration (`tools/tft-render/calibrate.py` + `seed_looks --calibrate`)

1. New example `crates/tft-vision/examples/seed_looks.rs`: composites renders into real frames and
   embeds them with the app's own `units::crop` + `tft-embed`:
   - backgrounds: quiet planning frames from the captures, at empty hexes (no bar within 120 px);
   - the render is scaled by `px_per_unit`, its feet placed on the hex, and a synthetic `SeenUnit`
     bar placed `bar_gap` px above the mesh top; the crop window (`units::CROP`, below the bar)
     then sees the unit as in game. No bar is drawn: the crop starts 30 px below it.
2. Grid search (only the units with ≥ 3 real `mine` crops): `pitch` 35–65° step 5,
   `px_per_unit` 5 values, `bar_gap` 4 values. Score: mean of (best seed sim to own real crops −
   best sim to other units' crops). Write `tools/tft-render/calib.json`.
3. Report the best setting's numbers and a side-by-side sheet of renders vs real crops.

Done when: `calib.json` is committed and the sheet shows renders sized and placed like real crops.

## T6. Full render and seed

1. With `calib.json`: render all units, 12 yaws × 2 sizes × 2 lights per variant (GPU, only with no
   game running; run in the background, resumable per unit).
2. `seed_looks` composites each render onto 2 random backgrounds (both arenas), embeds, keeps 16
   looks per unit by farthest-point picking, writes `fixtures/tft/looks_seed.bin` (TFL2, ~5 MB).
3. Drop variants that never win a real crop on the eval set (report which).

Done when: the seed file is committed with per-unit counts and the build includes it.

## T7. Combat and scouting use (`crates/tft-coach/src/scout.rs`)

1. Thresholds split: `SEED_SIM`, `SEED_MARGIN` beside the existing real-look ones (start values from
   T8's sweep; placeholders until then).
2. Vote per red bar across a fight's still reads: sum similarities per unit, name when the best
   leads by the margin.
3. Candidates: units allowed by that player's last trait panel (every trait of the unit present)
   and with copies left in the pool; unconstrained when no panel is known.
4. Tests with synthetic embeddings, as in `tests/scout.rs`.

Done when: tests, clippy clean; no change on my own board's tests.

## T8. Gate

1. New example `looks_eval`: loads the eval crops + `labels.jsonl`, the seed and (optionally) a learnt
   library, applies T7's rules, sweeps thresholds, prints per group: top-1, top-3, named %, wrong %
   at the chosen threshold, and the most confused pairs.
2. Set `SEED_SIM` / `SEED_MARGIN` at the point that meets ≤ 5 % wrong on `combat`; record named %.
3. Write the result into the spec (Status line + a short "Measured" section).
4. Pass → build and swap the app (no game running). Fail → report, and the next step is approach 2.

Done when: numbers are in the spec and the user has the verdict.

## Later (not in this plan)

Approach 2 (classifier head) and 3 (fine-tuned CNN) as written in the spec, reusing T1's eval set,
T6's renders and T5's compositing.
