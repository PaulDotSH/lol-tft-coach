# Screen geometry: self-calibration and health

Date: 2026-11-10. Status: draft for review.

## Goal

Every screen reader and overlay position either finds its element on the actual
screen or says it can't. No reader silently looks at the wrong pixels after a patch,
at another resolution or with a different HUD scale, and the user can see which
readers work.

Stopping condition: on a saved frame corpus at 1920×1080 and 2560×1440 with HUD
scale at minimum, default and maximum, every locator below either reports the
element within tolerance or reports "not found"; and in one real Mayhem game the
overlay's rank badges sit within 1 % of the window width of the card centres.

## Where geometry is fixed today (checked 2026-11-10)

| Reader | How it is placed | Self-locating? |
|---|---|---|
| Mayhem augment offer OCR | strip at fixed fractions of the client (`REGION_X` 0.18–0.82, `REGION_Y` 0.365–0.462 in `ocr/src/win/capture.rs`) | no |
| Mayhem overlay badges | `LOL_PITCH` 0.1867, `LOL_BADGE_Y` 0.33 in `overlay/mod.rs` | no |
| TFT overlay badges | `TFT_PITCH` 0.19, `TFT_BADGE_Y` 0.2 (guessed, per the README) | no |
| Tab scoreboard | Community Dragon source units (1600×1200) scaled by window size and centred, grid found by correlation within `SEARCH` 12 units | offset yes, scale no |
| Minimap | size fitted to `MinimapScale` from `PersistedSettings.json`; the map square found by its black padding | yes |
| TFT HUD (stage, gold, shop, HP list, plates, offers, trait rows) | pixel boxes measured at 2560×1440, default HUD scale, scaled by `width / 2560` | no |
| TFT units | HP bars found by colour; crop relative to the bar | yes (scale from width) |

Known unknowns: the LoL HUD scale setting for the scoreboard and offer cards, the
TFT HUD scale, and aspect ratios other than 16:9 (which HUD groups stay centred and
which hug an edge).

## Design

Three parts: locators (measure), a calibration store (remember), a health report
(tell). The readers themselves change only in where they take their rectangles from.

### 1. Locators

A locator takes a frame (or OCR boxes already made for the read) and returns where
an element actually is. Each lives beside its reader and is pure.

```text
Found { rect: Rect, scale: f32, score: f32 }           // scale relative to the expected size
trait-free: plain functions returning Option<Found>
```

- **Mayhem offer cards** (`ocr::offer::locate`, new). The offer read matches card
  titles from text lines (`ocr::scan` uses `recognize`); it switches to
  `recognize_boxes`, which gives the same text with positions. From the matched
  titles: each card's centre x and the title row's y. Overlay badges take x and y from these
  boxes instead of `LOL_PITCH` / `LOL_BADGE_Y`; the constants remain as the fallback
  for a card whose title wasn't read. When the strip reads nothing at a time an
  offer is expected (the known augment levels), one wider scan of the middle band
  (`WIDE_Y` 0.25–0.60) runs at most every `WIDE_EVERY` (5 s); if titles are found
  there, their row becomes the strip for this session and is stored.
- **TFT offer cards** (`tft_vision::offers`): the same from the "Choose One" title
  and the card title boxes the reader already has.
- **Tab scoreboard** (`ocr::scoreboard::Geometry::fit`). Today the grid search
  allows a shift. Add scale: find the left and right augment-grid columns
  independently; their distance over the expected distance is the scale, their
  midpoint the centre. `Geometry { s, cx, cy }` is then built from the fit rather
  than from the window size alone. Covers a non-default HUD scale.
- **Minimap**: already located; it only reports its result (square found or
  fallback used).
- **TFT HUD groups** (`tft_vision::layout`). The HUD is treated as five groups, each
  with its own scale and offset: top centre (stage), bottom centre (shop, gold, XP,
  level), right (player list, HP), left (trait panel), board (plates, units).
  Anchors found in the full-frame OCR pass the reader already does: the stage text
  ("N-N"), the shop's five name boxes, the player list's name column, the trait
  rows' name column. A group's transform is fitted from its anchors; a group without
  anchors on this frame keeps its stored or default transform. `Geo` becomes
  per-group.

A locator's result is accepted only inside a sanity range (scale 0.6–1.6 of
expected, centre within 10 % of the window) and above its own score threshold;
outside it the reader reports "not found" rather than reading there.

### 2. HUD settings as a prior

`clients::league_cfg` already reads `MinimapScale` and the flip option from
`PersistedSettings.json`. It also reads the HUD scale (spike: confirm the key name
in the file, and where the TFT client keeps its own) and passes it to the readers as
the starting scale, so the first frame of a session is usually right before any
locator has run.

### 3. Calibration store (`src-tauri/src/calib.rs`)

`%LOCALAPPDATA%\lol-coach\calib.json`:

```json
{"lol": {"2560x1440@1.00": {"patch": "16.22", "offer_strip": [0.18, 0.365, 0.82, 0.462],
          "scoreboard": {"s": 1.2, "cx": 1280, "cy": 720}, "seen": "2026-11-10"}},
 "tft": {"2560x1440@1.00": {"patch": "16.22", "groups": {"top": [1.0, 0, 0], "right": [1.0, 0, 0]}}}}
```

- Key: game, client size, HUD scale. A new key starts from the defaults.
- A stored value is used as the expected position next session; locators keep
  refining it (an exponential average, so one bad frame can't move it far).
- When the Data Dragon version changes, entries are marked unverified; the first
  successful locate re-verifies them. Until then the health report says "not checked
  since 16.22".
- Deleting the file is always safe.

### 4. Health report

- `AppState.readers: Vec<ReaderHealth>`:
  `ReaderHealth { name, state: Ok | Shifted | NotFound | Off | Unverified, detail, last_ok }`,
  e.g. "Augment offer: OK", "Tab scoreboard: found at 0.92× (HUD scale)", "TFT shop:
  not found since 16.23".
- Settings gets a "Screen reading" list with these rows; the header's source dots
  keep their meaning and turn amber when any enabled reader is `NotFound` for a
  whole game.
- A reader that has not found its element in a full game where it should have
  appeared logs one warning with the window size and HUD scale. In debug mode it
  also saves the frame, which is exactly the fixture needed to fix it.

### 5. Layouts from the game's own UI data (dev tool)

The Tab scoreboard was laid out from Community Dragon's UI data
(`KiwiScoreboard/UIBase`). `tools/layout/fetch.py` does the same for the other
scenes, as a dev step, not at runtime:

- spike: find the Mayhem augment selection scene and the TFT HUD scenes in
  `clientstates/gameplay/ux/`; if present, print each element's rectangle in source
  units;
- keep the printed layout per patch under `fixtures/layout/<patch>.json` and diff it
  against the previous patch. A change in an element a reader uses is the earliest
  warning that a constant needs a look, before any game is played.

If the scenes aren't there, the locators and the corpus still stand on their own.

### 6. Frame corpus and replay

- `fixtures/frames/<game>/<w>x<h>@<hud>/<element>.png`: small crops (not full
  screenshots) around each element plus a JSON line with the hand-checked rectangle.
  Sources: the debug dumps already written at 1440p; one practice-tool game and one
  TFT game per extra resolution and HUD scale with `run.cmd debug`. League in a
  1920×1080 window on the 1440p monitor is enough for the second resolution.
- `cargo run -p ocr --example calib -- <dir>` and the TFT equivalent run every
  locator over a debug session folder and print a table (found, scale, offset from
  expected). The same code runs as a test over the corpus.

## Order of work

1. Offer-card locator and badges from boxes (fixes the README's known limit on badge
   positions; smallest change, visible at once).
2. Health report with the readers as they are (states from what they already know).
3. Scoreboard scale fit.
4. Calibration store and HUD-scale prior.
5. TFT HUD groups (largest; needs the corpus first).
6. Layout fetch tool, if the spike finds the scenes.

## Errors and gaps

- Exclusive fullscreen stays unsupported (capture and overlay), as today.
- An ultrawide corpus needs an ultrawide capture; until one exists, 21:9 is reported
  as unverified rather than assumed to work.
- Locators run on reads that already happen. The only added capture is the wide
  offer scan, rate-limited and only when an offer is expected and unread.
- Nothing here sends input, takes focus or reads anything but the user's own screen.

## Testing

- Each locator over the corpus: found within tolerance at every resolution and HUD
  scale; "not found" on frames without the element (never a wrong rectangle).
- Synthetic: `scoreboard::Canvas` already draws a board for tests; draw it at 0.85×
  and 1.2× and off-centre, and `Geometry::fit` recovers scale and centre.
- `fake_league` at two window sizes: the offer locator's boxes match the drawn
  titles; overlay badge x within 1 % of the drawn card centres.
- Calibration store: unknown key gives defaults; patch change marks unverified; a
  single outlier moves the stored value by no more than the averaging step; a
  corrupt file is ignored.
- Health states: each transition (Ok to NotFound after a game without a find,
  Unverified to Ok on the first find).

## Implementation status (2026-11-10)

Built (order of work steps 1–4, branch `feat/geometry`):

- **Offer-card locator** `ocr::offer`: `STRIP`, `WIDE` (0.10–0.90 × 0.25–0.60), `TITLE_TOP`, `PITCH`,
  `rect`, `locate(boxes, names, at, client) -> Vec<Card>` (title centre x and top y as client
  fractions, a `Found` per card) and `strip_for(top, scale)`. The offer read uses `recognize_boxes`.
  A title's box comes from its matched words (the line box split by characters, so OCR-merged titles
  split and wrapped titles span both lines). Titles are kept on the row holding the most of them;
  scale = card pitch / default pitch. Sanity: 0.6–1.6, and a row of three centred within 10 %.
  `ocr::Found { rect, scale, score }` lives in `ocr`. `matching` returns the hit spans (`hits`).
- **Badges from boxes**: `overlay::lol_badges` puts each badge at its card's read x. The badge row
  keeps the default gap above the titles, scaled with the cards (`lol_badge_y`).
  `LOL_PITCH` / `LOL_BADGE_Y` stay as the fallback.
- **Wide scan**: the OCR thread reads `WIDE` at most every `WIDE_EVERY` (5 s). It does this while
  `Shared::offer_expected` is set (Mayhem, OCR on, `expected_augments(level)` > my augments, no
  cards on screen) and the strip reads fewer than two titles. A row found there becomes the stored
  strip, taken as is. Strip reads average into it.
- **Health report**: `model::{ReaderHealth, ReaderState}`, `AppState.readers`, and
  `src-tauri/src/health.rs`, with the readers "Augment offer", "Tab scoreboard" and "Minimap". When
  a reader in use misses a whole game it should have shown in, it is `NotFound` until its next
  find. That logs one warning per game with the window size and HUD scale. `health::degrade` turns
  the OCR / Minimap header dot amber in the backend, so the web header is unchanged. Settings shows
  a "Screen reading" list, and the mock fixtures carry one. In debug mode the frames are saved as
  `ocr-wide-miss` (wide band, nothing found) and `tab-wide-miss` (whole window, no board).
- **Scoreboard scale fit** `Geometry::fit(frame, origin, client, prior)`. Each augment column is
  searched separately, coarse then fine: 4 % scale steps over 0.6–1.6 of the window's default, a
  ±40 source-unit search, and a one-cell disambiguation, then 1 % steps. The column distance gives
  the scale and the midpoint gives the centre. The result is accepted only with ≥ 8 rows on the
  fitted grid and the centre within 10 %. Synthetic `Canvas` tests at 0.85× and 1.2×, and
  off-centre (+30/−20 px and −25/+15 px), recover the scale within 0.01 and the centre within 2 px.
  A plain frame gives `None`. `Geometry` fields are public.
- **Calibration store** `src-tauri/src/calib.rs` (`%LOCALAPPDATA%\lol-coach\calib.json`).
  Layout: game → `"<w>x<h>@<hud>"` → `{patch, offer_strip, scoreboard {s,cx,cy}, seen}`, where
  `seen` is in unix seconds. A new value moves the stored one by `ALPHA` = 0.2; a wide-scan find
  replaces it. An entry whose patch differs from the current one is unverified until the next find.
  A corrupt file is ignored, and writes happen at most every 30 s and at game end.
- **HUD-scale prior**: `clients::league_cfg` reads `GlobalScale` (HUD section, 0..1; this machine
  has 0.10) into `MinimapSettings::hud_scale`.
- **Minimap**: `ocr::minimap::find_square` (None means the fallback place is used). The minimap
  thread reports the result.
- **`cargo run -p ocr --example calib -- <dir> [manifest]`**: runs the offer locator, the board
  fit and the minimap square over a debug session's frames. On the 1440p sessions on disk,
  252 of 252 minimap frames were located.

Deviations:

- HUD scale is part of the calibration **key** and is logged with misses. It is not turned into a
  starting scale, because the mapping from `GlobalScale` to the scoreboard or card scale is unknown
  until the corpus exists. The first frame at a new key starts from the defaults.
- The scoreboard fit runs on the board capture that already happens, once an open board is read,
  at most every 10 s. Before the board has been found under the current key this session, the
  board can sit at a non-default scale where the column check misses it. For that case a
  whole-window fit runs at most every 5 s, only while enemy augments are unknown. This is a second
  added capture besides the wide offer scan.
- The fit's search covers the scaled prior centre ± 40 source units (about 2–3 % of the width),
  not the whole 10 % sanity range. The sanity range is still the acceptance rule.
- What counts as "should have appeared": the offer in a Mayhem game with OCR on after 90 s; the
  scoreboard once the coach asked for Tab (a heuristic, since the user may not have pressed it);
  the minimap whenever it was captured.
- `TITLE_TOP` = 0.40 is inferred from the measured "titles at y ≈ 41 %". No real offer frame is on
  disk to measure it.
- The TFT offer locator is not done: it needs positions added to `TftSnapshot` (tft-model and its
  bindings).

Open:

- In one real Mayhem game (`run.cmd debug`), check that the rank badges sit within 1 % of the
  window width of the card centres. Check `TITLE_TOP` against the logged card `y`
  (`ocr … cards [(id, x, y)]`), and that `calib.json` gets a `lol` entry with a stable
  `offer_strip`.
- In a Mayhem game, open Tab and check that the "Tab scoreboard" row turns Ok and that
  `calib.json` holds a `scoreboard` close to `Geometry::new`. Then change the HUD scale, open Tab,
  and check that the wide fit finds the board ("scoreboard found at …×") and the row shows
  Shifted.
- The debug frame cap (`MAX_FRAMES` 300) was used up by minimap frames in every session on disk,
  so no `ocr-offer` or `tab-board` frames were saved. Record with gank warnings off, or raise the
  cap, to get offer and scoreboard frames.
- Step 5 (TFT HUD groups) and step 6 (layout fetch tool) are not started. The TFT client's own HUD
  scale setting is not located either.
- Frame corpus `fixtures/frames/…` at 1920×1080 and 2560×1440 with HUD scale minimum, default and
  maximum needs real recordings. After that, run `--example calib` over them and turn the table
  into a test.
- 21:9 stays unverified.
