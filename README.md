# LoL Coach

Second-monitor League of Legends coach: what to buy next and why, who the
threats are, how you compare, champ select picks/bans/build, and ARAM: Mayhem
augment ranking read off the screen. Read-only (Riot's local APIs + public
stats sites), silent, never an overlay. Spec: `docs/superpowers/specs/`,
future ideas: `docs/ideas.md`.

## Run

```
target\release\lol-coach.exe
```

Start it once; closing the window hides it to the tray (tray → Quit to exit).
Run League in **borderless** mode for Mayhem augment OCR.

Build:

```
npm --prefix web install
web\node_modules\.bin\tauri build --no-bundle      # → target\release\lol-coach.exe
web\node_modules\.bin\tauri dev                    # hot-reload dev
```

## Layout

| Crate | Job |
|---|---|
| `crates/model` | Shared types; `cargo test -p model` regenerates `web/src/lib/bindings/*.ts` |
| `crates/data` | Data Dragon (+ `traits.toml`), lolalytics `mega` API + build-page benchmarks, u.gg Mayhem JSON; disk cache |
| `crates/clients` | League client (lockfile, champ select, end-of-game) and Live Client Data API |
| `crates/coach` | Pure decisions: threats, needs, item scoring, matchups, self stats, timers, augments, champ select, recap. Tuning in `consts.rs` |
| `crates/ocr` | GDI capture of the League window → Windows OCR → fuzzy augment matching |
| `src-tauri` | Phase loop (`orchestrator.rs`), background fetch cache, commands, tray |
| `web` | Svelte 5 dashboards; mock mode when opened outside Tauri (`npm --prefix web run dev`, `?mock=<fixture>`) |

Files: settings `%APPDATA%\lol-coach\settings.toml`; cache, logs, recaps and
recorded fixtures under `%LOCALAPPDATA%\lol-coach\`.

## Testing

- `cargo test --workspace`: parsers over saved real responses, coach behaviour tests.
- `cargo run -p lol-coach --example smoke`: real Data Dragon / lolalytics / u.gg data through the coach, printed (no UI, no game).
- Fake in-game: serve `fixtures/clients/live_allgamedata.json` over HTTPS on `127.0.0.1:2999` and the app goes in game.
- `cargo run -p ocr --example fake_league -- "Big Brain" "Quickstep" "Warlock Juicebox"`: a topmost, never-activated stand-in League window to test OCR end to end.
- `cargo test -p ocr --release -- --ignored --nocapture live_capture`: during a real game, prints what OCR reads.
- Settings → "Record games as test fixtures" saves live snapshots every 30 s.

## Known limits (v1)

- Champ select and end-of-game parsing are built against hand-made fixtures; first real games may need field fixes (turn on recording).
- Objective timers and threat/item weights are first guesses (`crates/coach/src/consts.rs`).
- Mayhem items use lolalytics ARAM data (lolalytics has no Mayhem; u.gg's Mayhem item samples are tiny).
- No vision benchmark source; per-role constants are used.
