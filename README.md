# LoL Coach

Second-monitor League of Legends coach: what to buy next and why, who the
threats are, how you compare, champ select picks/bans/build, and ARAM: Mayhem
augment ranking read off the screen. Read-only (Riot's local APIs + public
stats sites), silent, never an overlay. Spec: `docs/superpowers/specs/`,
future ideas: `docs/ideas.md`.

## Features

- **Champ select**: counter-picks (all champions in your role) blended with duo synergy and comp gaps, bans, draft breakdown for both teams, team composition (damage mix, frontline, engage, peel, CC), ARAM/Mayhem bench ranked by tier + team fit, build plan.
- **In game**: Buy next with threat-aware reasons, gold ETA and back plans, shopping list (emphasised while dead), full path, late-game gold sinks; threat board with R-online/base ult cooldowns, finished-item spikes and enemy Mayhem augments; death windows; item-gold diff (team, lanes, sparkline); self-coach row with goals; effective HP vs the enemy damage mix and pen choice; objective timers.
- **Mayhem**: augment offers read by OCR and ranked for your champion (ranks only, never win rates), picked-augment detection, augment-aware item scoring, mode rules (`crates/coach/modes.toml`), "hold Tab" hints and enemy augments read from the Tab scoreboard.
- **Between games**: last-game recap with death review, recent-game loss patterns (macro coach), trends and goals, champion lookup, settings.

## Run

```
run.cmd            # start (builds first if needed)
run.cmd debug      # debug mode: verbose log + dumps, follows the log in the console
run.cmd build      # rebuild, then start (combine with debug)
```

Debug mode writes everything to `%LOCALAPPDATA%\lol-coach\debug\<session>\`:
`lol-coach.log` (debug level, threads, source lines, frontend errors, panics with
backtraces), raw LCU dumps (champ select, end of game, a one-time probe of the
match-history/summoner/gameflow endpoints), Live API snapshots (and any that fail
to parse), advice snapshots, and PNGs of augment offers and the Tab scoreboard as
the game shows them. Each kind is written only when it changes (rate-limited,
capped per session). It also opens WebView2 debugging on `127.0.0.1:9333`.

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
| `crates/model` | Shared types: re-exports the `model-*` crates below and holds the app state. `cargo test -p model*` regenerates `web/src/lib/bindings/*.ts` (the ts-rs derive is compiled only for tests and the `ts` feature, not in app builds) |
| `crates/model-core` | Ids, `Lane` / `Mode` / `Team` / …, `ItemPick`, the `data!` / `enums!` type macros |
| `crates/model-static` | Data Dragon items, champions, runes; lolalytics and u.gg stats (what `data` produces) |
| `crates/model-game` | Live snapshot, champ select, end of game, rank, match history (what `clients` produces) |
| `crates/model-live` / `model-draft` / `model-review` | `coach` output: in-game advice / champ select advice / recap, self-coaching, game review |
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
- Match history, Tab scoreboard geometry and augment-pick detection are built from docs, one third-party screenshot and fixtures; check them in real games (`cargo test -p clients --test history -- --ignored --nocapture live_history` with the client open).
- Parallel worktrees: the global cargo `build-dir` is shared, so give each worktree its own `CARGO_BUILD_BUILD_DIR`.
