# TFT unit recognition from 3D renders

Date: 2026-10-25. Status: design agreed in a brainstorm with the user; spec awaiting review.

## Goal

Name other players' units where it matters for contested units and threats, **enemies in combat**
and **scouted boards**, without the user playing many games to collect data. Recognition is by
appearance: units carry no text on screen (OCR covers only the shop, trait panel and tooltips,
which the coach already reads). Stars, positions and bench slots already come from HP bars.

## Why

The contested-units work (`feat/tft-contested`, merged in `dccbaae`) names opponents' units by
looks from a library learnt off my own confirmed pieces. On the two captured games it named 0
opponent units: the library held 11 looks of 5 units after a whole game, and similarity across
arenas was low. A library seeded with every unit of the set, before any game, removes that limit.

## Decisions

| Topic | Decision |
|---|---|
| Data | 3D renders of the Set 18 models from Community Dragon. Riot API final boards as optional ground truth (key steps in `docs/riot-api-key.md`). Streamer VODs not used. |
| Scope | Enemies in combat and scouted boards. My own board stays on purchases + trait panel + learnt looks. |
| Approach | **1: render-seeded looks library** (DINOv2 embeddings of renders, no training). Approaches 2 and 3 are written below for a later comparison on the same test set. |
| Target | On combat-start enemy crops: ≥ 60 % named, ≤ 5 % named wrong (at the naming threshold, with the trait panel / pool constraints). |

## Facts (checked 2026-10-25)

- `raw.communitydragon.org/latest/game/assets/characters/` has 102 `tft18_*` folders (champions,
  summons such as `tft18_azirsoldier`, PvE such as `tft18_brambleback`).
- Each has `skins/base/` with a `.skn`, one colour texture `*_tx_cm.png`, and a folder of OBJ
  meshes, one per material, e.g. `tft18_ahri_18/`: `Ahri_Body_MAT.obj`, `Ahri_Tails_MAT.obj`,
  `Fox_MAT.obj`, `MidForm_MAT.obj`, `Snow_MAT.obj`, `VFX_Sleeves_MAT.obj`.
- No `.skl` / `.anm` there: renders are in the **bind pose**, not the idle or attack poses seen in
  game. This is the main risk.
- The set data (`fixtures/tft/cdragon_set18.json`) has each unit's `characterName` and square icon.
- The app's unit crop is bar-relative: `units::CROP` = (−18, 30, 82, 150) px at 2560 width from the
  HP bar's left end, resized to 112 and embedded by `tft-embed` (CLS + Gaussian-weighted patch mean,
  768-d).
- Blender 5.2 with HIP (RX 9070 XT) is installed; `blender -b -P script.py` renders without a window.

## Design

### 1. Assets (`tools/tft-render/fetch.py`)

- Reads the Set 18 units from the set data, maps `characterName` to the Community Dragon folder,
  downloads the OBJs and texture into a cache outside the repo
  (`%LOCALAPPDATA%\lol-coach\dev\render\assets\<unit>\`).
- Variants per unit: all meshes together, plus each large mesh group alone when it looks like a
  form (Ahri's Fox / MidForm). Tiny VFX meshes (`VFX_*`) are dropped. Which variants stay is
  decided by the evaluation (§5), not guessed.
- Summons and PvE units are included only if they show HP bars in game (Azir soldiers, crabs).
- A unit without a texture or mesh is listed and skipped.

### 2. Rendering (`tools/tft-render/render.py`, run in Blender)

- Camera: TFT's fixed pitch and focal length, **calibrated** on the captures. For the units with
  confirmed real crops (board_replay labels), a grid search over pitch, scale and vertical offset
  picks the setting where renders' embeddings are closest to the real crops'.
- Per variant: 12 facings (30° steps) × 2 sizes (★1, and the ★2/★3 scale-up) × 2 lights = 48 renders,
  transparent background, EEVEE on the GPU.
- Lighting matched loosely to the arena (warm key, soft fill); no post effects.

### 3. Compositing and embedding (`cargo run -p tft-vision --release --example seed_looks`)

- Each render goes onto empty-hex patches cut from the two captured games (both arenas), with a
  fake HP bar drawn where the game draws it relative to the unit's top.
- The composite is cropped with the **same** `units::crop` code and embedded with the same
  `tft-embed`, so preprocessing can't drift between the seed and the live reader.
- Per unit, keep ~16 diverse looks (farthest-point picking on the embeddings) and write
  `fixtures/tft/looks_seed.bin`: about 100 units × 16 × 768 at f16, ~5 MB. New format version
  `TFL2` (f16, seed flag); `TFL1` files still load.

### 4. In the app

- `Library` loads the built-in seed (`include_bytes!`) plus the learnt `looks.bin`. Seed looks are
  marked: they name units but are never learnt over, deduplicated away or saved to `looks.bin`.
- Separate thresholds for seed and real matches, tuned on §5's test set. Real looks keep priority
  when both match.
- Combat and scouting already read at combat start with full bars and two still reads
  (`scout.rs`, `units::foes`). Added: a vote per red bar across a fight's still reads, and
  candidates limited to units allowed by that player's last trait panel and the unit pool.

### 5. Evaluation (`cargo run -p tft-vision --release --example looks_eval`)

- Test set from the two captured games:
  - my confirmed pieces (board_replay labels);
  - hand-labelled combat-start enemy crops and the few scouted-board crops;
  - optionally, Riot API final boards for those two matches (exact units, stars and items of every
    player in the last rounds), once the user has a key.
- Reports top-1 / top-3, the named rate and the wrong rate at the threshold, per group (mine,
  scouted, combat), and a confusion list per unit.
- Gate: the target in Decisions. Below it, approach 2 is next.

## Later comparison (not built now)

Both reuse the renders, composites and the §5 test set.

### Approach 2: classifier head on DINOv2 features

1. Export the composites' DINOv2 features (the Rust example writes `.npy`).
2. Augment before embedding: real effect overlays cut from combat frames (flashes, burns, shields,
   other units), blur, colour jitter, partial occlusion.
3. Train a linear layer or small MLP (768 → ~100 units) in PyTorch on CPU, with a reject class
   from empty hexes and PvE.
4. Ship `head.bin` (~300 KB); Rust applies it with one matrix multiply after the embedding.
5. Score on §5; compare with approach 1 at the same wrong rate.

### Approach 3: fine-tuned small CNN

1. Use the same composites with heavier augmentation (as in 2), at 112–160 px.
2. Fine-tune MobileNetV3-small (ImageNet weights) end to end on the RX 9070 XT via ROCm (see the
   ACE-Step ROCm setup notes), or CPU if ROCm misbehaves.
3. Export to ONNX; load through `tft-embed` as a second model (`Embedder::load` already takes a path).
4. Score on §5; weigh accuracy against the extra model size and per-read time.

## Risks

- **Bind pose vs idle pose.** If renders don't land near real crops in embedding space, approach 1
  fails the gate; that is what approaches 2 and 3 are for.
- Mesh groups may not map cleanly to forms or star looks.
- Community Dragon could miss textures for some units; listed by `fetch.py`.
- Seed size in the exe (~5 MB): acceptable; could be fetched instead if it grows.

## Out of scope

Streamer VODs, my own board's recognition (already covered), item recognition (done in
`feat/tft-items`), any change to how HP bars, stars or the trait panel are read.
