# LoL Coach — design

Date: 2026-10-22
Status: draft, awaiting review

## Purpose

A desktop app that sits on the second monitor while you play League of Legends
and tells you what to build and why, based on lolalytics statistics and the live
state of the game: who the enemies are, who is fed, who keeps killing you. It
also helps in champion select (picks, bans, build plan) and between games
(last-game recap, champion lookup).

Success looks like: glance at the second monitor while backing, see one clear
"buy next" with a reason that matches what is happening in the game, and see at
a glance which enemy is the threat and how you compare to your lane opponent.

## Decisions (from the design interview)

| Topic | Decision |
|---|---|
| Display | Normal window on a second monitor. Never takes focus, never always-on-top, no overlay. |
| Lifecycle | Launched manually, then lives in the tray between games; remembers window position. |
| Alerts | None. Silent; the user glances when they want. |
| Layout | One dashboard per phase (champ select / in game / idle), no tabs to click mid-game. |
| Modes | Summoner's Rift (ranked solo/flex, normal draft) and ARAM. No Arena. |
| Stats bracket | Setting, default Emerald+, region all. |
| Patch window | Current patch; falls back to last 30 days while the current patch's sample is small. |
| Champ select | Counter-pick suggestions, ban suggestions, build plan once locked. |
| Pick pool | **All champions** played in your role (changed 2026-10-22, was "my pool"). |
| Bans | Blend: counters to your hovered champ, then meta threats in your lane. |
| ARAM: Mayhem | First-class: augment advice from u.gg (bigbrain CDN) per champion, read off screen with Windows OCR (auto-scan, borderless). Bench/reroll ranking in champ select. |
| Item advice | "Buy next" + 1–2 alternatives on top; full 6-item late-game path below. |
| Stats vs situation | Balanced: items 1–2 follow lolalytics; boots and items 3–6 swing toward threats. |
| Threats | All enemies scored by how fed they are; extra weight for whoever kills you and your lane opponent. |
| Strength estimate | Item gold value + KDA + level + CS vs lobby average; numbers shown. No live damage exists. |
| Self-coach (live) | CS/min vs benchmark, item gold & level vs lane opponent, deaths & killers, vision score. |
| ARAM | Items + threat board + self-coach; no lane matchups or objective timers. |
| Idle screen | Last-game recap, status & settings, champion lookup. |
| Stack | Tauri 2, Svelte 5 + TypeScript + Vite (same toolchain as chess-coach). |
| Location | `repos/apps/lol-coach` |

## Data sources

All verified on 2026-10-22 unless marked **spike**.

### Live Client Data API (in game)

`https://127.0.0.1:2999/liveclientdata/allgamedata`, self-signed certificate.
The HTTP client accepts invalid certificates **only** for `127.0.0.1`.

Gives every player's champion, items, level, K/D/A, CS, ward score, team and
`position` (matchmade games), the active player's gold, stats and runes, and
the event list (kills with killer/victim/assisters, dragon/baron/herald kills
with timestamps, turrets, game end). It gives **no enemy gold and no damage
numbers**, so strength is estimated (see Coach).

Returns errors / 404 while the game is loading; that is the "loading" state,
not a failure.

### League Client (LCU) API (champ select, recap, pool)

Credentials come from `C:\Riot Games\League of Legends\lockfile`
(`name:pid:port:password:protocol`, basic auth `riot:<password>`), found via
the running `LeagueClientUx.exe` process if the install path differs.
Read-only calls:

- `/lol-gameflow/v1/gameflow-phase`, `/lol-gameflow/v1/session` — phase and queue id (ARAM = 450).
- `/lol-champ-select/v1/session` — my cell, assigned position, hovers, picks, bans, revealed enemy picks.
- `/lol-summoner/v1/current-summoner` — who you are (recap).
- `/lol-end-of-game/v1/eog-stats-block` — real post-game damage, gold, CS, vision for the recap.

Polled at 1 s during champ select, 5 s otherwise. (The LCU websocket is a
later optimisation, not needed for v1.)

The app never shows information champ select hides (no de-anonymised names).

### lolalytics (undocumented)

`https://a1.lolalytics.com/mega/?ep=<ep>&v=1&patch=<p>&c=<slug>&lane=<lane>&tier=<tier>&queue=<queue>&region=all`

- `patch` = `16.21` (current) or a day count (`7`, `30`).
- ARAM: `queue=aram`, **omit `lane`**.
- `ep=build-itemset` — `itemSets.itemSet1..5`, `itemBootSet1..6`: rows `[item_id, n1, n2]`. **Spike:** confirm which of n1/n2 is games vs wins by comparing with the website.
- `ep=build-earlyset` — starting item sets `[ids_joined_by_underscore, wr, pr, n]`.
- `ep=counter` — overall `stats` (wr, pr, analysed, lane distribution) and `counters[]` (`cid`, `vsWr`, `n`, `d1`, `d2`, `allWr`) for the champ vs every opponent.
- `ep=build-team` — teammate synergy (`team_h` header + rows per lane). Not used in v1 except as optional pick tiebreak.
- Build page HTML (`lolalytics.com/lol/<slug>/build/`) embeds a Qwik state blob (`<script type="qwik/json">`) containing per-champion averages (`gold`, `minions`, `damage`, `damageTaken`, …). **Spike:** decode the Qwik ref format well enough to extract these benchmarks and the core-build table.
- **Spike:** tier list source for meta bans (tier list page or an endpoint).

Rules: one request at a time, disk cache with 12 h TTL keyed by the full query,
browser-like User-Agent, prefetch only what the current phase needs. If any
call fails or the format changes, that source is marked degraded in the UI and
the coach runs on rules + Data Dragon only.

### u.gg ARAM: Mayhem data (bigbrain CDN)

lolalytics has no Mayhem data (unknown `queue` values silently fall back to
generic data). u.gg serves static JSON, patch in `16_21` form:

- `static.bigbrain.gg/custom-aram-mayhem/<p>/aram-mayhem-augment-manifest-<p>.json` — augment id → name (594 entries, includes Arena ids).
- `.../tierlist-champions-<p>.json` — champion tiers `S+ … D` (champion keys).
- `.../tierlist-per-champion-augments-rarity-<p>/tierlist-augments-<key>-<p>.json` — per champion, `rarities.kPrismatic|kGold|kSilver`: augment ids ordered best → worst.
- Icons: `static.bigbrain.gg/cdragon-custom/<p>/augments/<id>.webp`.

Mayhem items use lolalytics ARAM (`queue=aram`) + threat logic; u.gg's Mayhem
item samples are too small.

### Screen OCR (Mayhem augments)

During games on the ARAM map, every ~1.5 s: find the League window
(`RiotWindowClass`), BitBlt the centre region of its client area (borderless
mode), run `Windows.Media.Ocr` (en-US, installed), fuzzy-match recognised lines
against the augment manifest. When ≥2 augments match, that is an offer: rank
them by the champion's u.gg list (position within rarity) and show the pick on
the dashboard. No input is sent to the game; nothing is drawn over it.

### Data Dragon / static data

`ddragon.leagueoflegends.com` (`api/versions.json`, `item.json`,
`champion.json`, images). Cached per patch.

- Items: cost, build path (`from`/`into`), stats, tags (`Armor`, `SpellBlock`,
  `Tenacity`, `ArmorPenetration`, …), Summoner's Rift / ARAM availability. Anti-heal,
  stasis, spell shield, etc. detected from the description text where
  possible, with a small hand-written override table.
- Champions: key ↔ slug ↔ lolalytics `cid` (same numeric key), tags
  (Assassin, Mage, Marksman, Tank, …), `info.attack/magic` for damage type.
- `traits.toml` (shipped, hand-curated): per-champion flags Data Dragon lacks —
  heavy healing, heavy CC, true damage, on-hit, burst. Unlisted champions fall
  back to tag-derived defaults.

## Architecture

Rust backend (Tauri core) with five modules, each with one job, plus a thin
orchestrator. The frontend only renders.

```
lcu ──────────┐
live ─────────┤
static_data ──┼──> orchestrator ──> coach (pure) ──> Advice ──Tauri event──> Svelte UI
lolalytics ───┘         │
                         └── settings, cache, recorder
```

| Module | Responsibility | Interface (sketch) |
|---|---|---|
| `lcu` | Locate lockfile, authenticated read-only calls | `Lcu::connect() -> Option<Lcu>`, `phase()`, `champ_select()`, `masteries()`, `eog_stats()` |
| `live` | Poll Live Client API, normalise | `poll() -> LiveState` (`Unavailable` / `Loading` / `InGame(GameSnapshot)`) |
| `static_data` | Data Dragon + traits, per-patch cache | `StaticData::load(patch) -> StaticData` with `item(id)`, `champ(key)` |
| `lolalytics` | Endpoint calls, parsing, cache, patch fallback | `champ_stats(champ, lane, queue) -> ChampStats` (item sets, early sets, counters, benchmarks) |
| `coach` | All decisions; no I/O, no clock | `champ_select_advice(&CsInput) -> CsAdvice`, `game_advice(&GameInput) -> GameAdvice`, `recap(&RecapInput) -> Recap` |

The orchestrator runs a phase state machine
(`NoClient → Idle → ChampSelect → Loading → InGame → PostGame → Idle`), drives
the sources at the phase's rate, calls `coach`, and emits `advice` events.
The UI never calls sources directly; settings changes go through Tauri commands.

Storage: `%APPDATA%\lol-coach\settings.toml`, `%LOCALAPPDATA%\lol-coach\cache\`,
`%LOCALAPPDATA%\lol-coach\recaps\` (last N game recaps as JSON).

## Coach

### Threat score (Rift and ARAM)

For each enemy, compare to the lobby average:

- item gold value (sum of Data Dragon costs of their current items),
- KDA contribution `(kills + 0.5·assists − deaths)`,
- level difference,
- CS difference (role-adjusted: supports and junglers compared within role).

Combine into a 0–100 score (weights in a constants file, tuned with recorded
games). Multiply by personal relevance: `×(1 + 0.25·times_they_killed_me)`
(capped) and `×1.3` for the lane opponent. The threat board shows the rank,
the score, and the raw numbers behind it ("7/1/2 · +2.1k item gold · +2 lvl ·
killed you 3×").

### Needs vector

From the enemy team weighted by threat score: physical vs magic vs true damage
share, burst/assassin pressure, healing, heavy CC, tankiness (armor/MR/HP
items bought), crit/attack-speed carries. Enemy items shift the profile (an
enemy Kayle building AP counts as AP).

Output: weights for answers such as `armor`, `mr`, `anti_heal`, `tenacity`,
`cleanse`, `anti_burst` (stasis, GA, shields), `armor_pen`, `magic_pen`,
`anti_crit`, `anti_attack_speed`.

### Item scoring

Candidates for slot k = lolalytics `itemSet<k>` rows with enough games, plus
situational items valid for the champ's class (from Data Dragon tags) that
answer a need. Excluded: owned items, items that conflict with owned ones
(boots, unique groups), wrong map.

`score = stats_score + slot_weight(k) · situational_score`

- `stats_score`: Wilson lower bound of win rate, small pick-rate bonus.
- `situational_score`: item's answer vector · needs vector.
- `slot_weight`: low for items 1–2, high for boots and items 3–6 (balanced mode).

"Buy next" = the top item for the next empty slot, with a one-line reason built
from its largest score term ("Zhonya's: Zed is 7/1 and has killed you 3×").
If current gold is short, it names the best component to buy now toward it.
The late-game path fills the remaining slots greedily, re-scored after each
pick, and includes boots and situational swaps late (e.g. sell boots, elixirs).

### Matchups (Rift only)

From `counter` for your champ: win rate vs each enemy in their likely lane,
your lane opponent highlighted, plus a short "you're favoured / even /
unfavoured" label. Lane opponent = Live API `position` match, else champ
select assigned position, else lolalytics lane distribution.

### Self-coach (live)

- CS/min vs lolalytics benchmark for champ + rank, with "N CS behind/ahead of
  average at this time".
- Item gold value and level vs lane opponent.
- Deaths and who killed you (from kill events), with the defensive suggestion
  feeding back into the threat weight.
- Vision score vs a role benchmark (**spike**: lolalytics may not have vision;
  else constants per role).

Shown as a compact row, numbers first, no nagging text.

### Objective timers (Rift only)

From kill events: dragon, baron, herald (and any current-season objectives)
next-spawn times. Respawn constants live in a config file because they change
between seasons (**spike:** confirm current values in a real game).

### Champ select

- **Counter-picks**: for each champ with ≥5 % of its games in your role, score =
  weighted mean of `vsWr` against revealed enemies (lane opponent weight 1.0,
  others 0.3), blended with the champ's overall win rate. Top 3 with reasons.
- **Bans**: blend of (a) champs that beat your hovered champ / pool hardest,
  weighted by their pick rate in your lane, and (b) meta threats in your lane.
- **Build plan** after lock-in: starting items (`build-earlyset`), core build
  and boots against the revealed enemy comp, using the same item scoring with
  threat scores replaced by "unknown, assume average".
- ARAM / Mayhem champ select: build plan, plus your champ and the bench
  champions (`benchChampions` in the session) ranked by Mayhem/ARAM tier, so you
  know whether to swap or reroll.

### Recap (idle)

On `EndOfGame`, store the last snapshot and the advice timeline; fetch
`eog-stats-block` for real damage/gold/CS/vision. Recap shows your stats vs
benchmarks and your final build vs what was recommended at each back.

## UI

One window, three dashboards switched automatically by phase.

- **In game**: Buy next (large) + alternatives · late-game path row · threat
  board (enemy rows sorted by score) · self-coach row · matchups (Rift) ·
  objective timers (Rift) · source status dots (client / game / lolalytics).
- **Champ select**: suggested picks · suggested bans · build plan once locked.
- **Idle**: last recap · champion lookup (search a champ + lane, see its
  build, counters, early items) · settings (rank, patch mode, pool editor,
  window) · connection status.

Readable at a glance from a second monitor: large type for Buy next, item icons
from Data Dragon, minimal text. Dark theme.

## Error handling

- No client / no game: status shown, sources idle.
- Live API loading: "Loading…" state, not an error.
- lolalytics failure or format change: degraded badge, coach falls back to rules
  + Data Dragon, cached data used if present.
- Data Dragon failure: last cached patch.
- Every source failure is logged to `%LOCALAPPDATA%\lol-coach\logs\` with the
  request and response status.

## Testing

- `coach` unit tests over fixture inputs: recorded `allgamedata` snapshots,
  saved lolalytics responses, saved Data Dragon subsets. Assertions on
  behaviour ("fed AP assassin with 3 kills on me → stasis item in buy next").
- Parser tests for each lolalytics endpoint and the Qwik blob, from saved
  responses.
- A **recorder** (debug setting) saves live snapshots and champ select sessions
  during real games, to grow the fixture set.
- Frontend: Vite dev mode with mock advice fixtures (like chess-coach's
  `dev:mock`), checked in a browser pane; no tests that grab focus or click on
  the user's desktop.

## Policy notes

Read-only: no input automation, no memory reading, only Riot's public local
APIs. Nothing hidden by champ select is revealed. No enemy summoner-spell or
ultimate timers. lolalytics is used through its own public endpoints, politely
cached; if they object or block it, the app degrades to rules-only.

## Out of scope for v1

Arena; overlay; sounds/toasts; auto-start with Windows; runes and skill order
(possible v2: lolalytics has them); LCU websocket; post-game damage graphs;
multiple accounts.

## Spikes before or during implementation

1. `build-itemset` row format (games vs wins).
2. Qwik state decoding for benchmarks and core build.
3. Tier list source for meta bans.
4. Vision benchmark source.
5. Current objective respawn timers.
6. Exact LCU champion-mastery endpoint on the current client.
