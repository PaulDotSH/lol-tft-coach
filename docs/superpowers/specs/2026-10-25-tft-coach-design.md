# TFT coach (Set 18 + Choncc or Treat)

Date: 2026-10-25. Status: agreed in a grilling session with the user; build approved.

## Goal

The same coach app, on the second monitor, coaches Teamfight Tactics: live advice during
the game read from my own screen, a pre-game meta view, a post-game review, and trends
with TFT LP. It supports the current set (Set 18, Enchanted Wilds, patch 18.5) in ranked
and normal games, and the limited-time mode **Choncc or Treat** (queue 1230,
2026-10-21 → 2026-11-17) as a separate crate that is deleted after the event.

## Decisions (from the grilling)

| Topic | Decision |
|---|---|
| Policy | Full live advice. Riot's TFT developer policy forbids real-time, game-state advice and scouting in published apps; this is a personal tool that reads only my own screen (no memory reads, no input automation). Accepted by the user. |
| App shape | Same app. In a TFT queue the window switches to TFT screens; the LoL coach already goes quiet (`not_lol`). |
| Parts | Live advice first; then pre-game meta, post-game review, trends + TFT LP. |
| Live reading | Screen text (stage, gold, level, XP, HP, streak, team size, shop names, offered augments, trait panel), owned units from shop-purchase tracking checked against the trait panel, item components by icon matching. |
| Scouting | Yes: when I view another board, name its units (looks, then the trait panel's set search) and, in my fights, my opponent's red-bar units (looks); kept per player by name plate. Flag units in my target comp that others hold: copies held and left in the pool; the trait-panel guess where units go unnamed. |
| Advice | Shop buy/skip (+ contested), level/roll timing, pivots + item builds + holder, augment ranking. |
| Target comp | Fully automatic: best fit from MetaTFT comps given owned units, components, augments. |
| Voice | Key moments only, each once: level / roll-down timing, a contested key unit, augment pick. |
| Queues | Ranked 1100 + normal standard, and Choncc 1230. No Double Up / Hyper Roll. |
| Data | MetaTFT unofficial JSON (comps, unit/item/trait/augment stats) + Community Dragon `cdragon/tft/en_us.json` (Set 18 static data, icons). |
| Crates | `tft-model`, `tft-data`, `tft-coach`, `tft-vision`, `tft-choncc`; thin `tft` module in `src-tauri`. LoL `model` crate split in parallel. |
| Execution | Spec + `tft-model` first (this commit), then agents in worktrees: tft-data, tft-coach + tft-choncc, UI, LoL model split. tft-vision by the main session with live calibration in the user's TFT games (screenshots saved to the debug folder only). |

## Facts the design relies on (checked 2026-10-25)

- LCU TFT history: `GET /lol-match-history/v1/products/tft/{puuid}/matches?begin=0&count=N`
  → `{games: [{json: {gameId, gameCreation, game_length, queueId, tft_set_number,
  tft_game_type, participants: [{puuid, placement, level, last_round, gold_left,
  traits[{name, num_units, style, tier_current, tier_total}], units[{character_id,
  itemNames[], rarity, tier}]}]}}]}`. **No augments** (Riot removed them after Set 13).
- LCU ranked: `queueMap.RANKED_TFT` (user: Emerald IV 53 LP) and `RANKED_TFT_DOUBLE_UP`.
- Set 18 API names carry a `DA_` prefix in every queue (`DA_18_Lux`, `DA_GuinsoosRageblade`,
  `DA_18_Defender`); it is not Choncc-specific. Choncc is identified by queue 1230.
- Community Dragon `latest/cdragon/tft/en_us.json` (24 MB): `setData[]` with
  `{number: 18, mutator: "TFTSet18", champions[91], traits[36]}`; `items[4231]` including
  `TFT_Item_ChonccsCrown`, `TFT_Item_ChonccsChalice`, augments and `DA_*` items. Cache it
  like Data Dragon (version-keyed, on disk); keep only Set 18 entries in memory.
- MetaTFT (`https://api-hc.metatft.com`):
  - `/tft-comps-api/comps_data?queue=1100` → `results.data.cluster_details{id: {units_string,
    traits_string, name[], overall{count, avg}, stars[], builds[{unit, buildName[3], avg,
    count}], top_itemNames[], levelling ("lvl 7"/"lvl 8"/…), difficulty}}` (60 clusters).
  - `/tft-comps-api/comps_stats?queue=…&patch=current&days=2&rank=…` → per-cluster
    placement histograms.
  - `/tft-stat-api/units?queue=Q`, `/items?queue=Q`, `/traits?queue=Q` → `{results:
    [{unit|itemName|trait, places[8]}]}`; **queue 1230 returns its own numbers** (Choncc
    stats exist for units).
  - Augment stats endpoint not found yet (`/tft-stat-api/augments` → 500). tft-data
    finds it (browser network panel of metatft.com) or falls back to augment tiers
    from comps (`top_augments`) / none.
- The live client API (port 2999) in TFT is unverified; the vision crate is the primary
  source. First live session checks what 2999 returns in TFT.

## Architecture

```text
tft-model   types only (serde + ts-rs → web/src/lib/tft-bindings/)
   ↑
tft-data    Community Dragon set data, MetaTFT meta, LCU TFT history parsing (pure parsers + fetch)
tft-vision  screen reading → TftSnapshot (uses crate `ocr` for capture + Windows OCR)
tft-coach   pure: Tracker (owned units from snapshots), comp fit, shop/econ/items/augments, calls,
            review, trends; ModeHooks trait
tft-choncc  ModeHooks for queue 1230 (rules, starting-carry comps, orb/item priorities)
   ↑
src-tauri/src/tft.rs  orchestration: phase/queue, fetches (cache), vision loop, coach, emits "tft"
web/src/tft/          TFT screens; web/src/lib/tft-store.svelte.ts listens to "tft"
```

- The TFT state goes to the UI on its own event `tft` (not inside LoL `AppState`), so
  TFT and the LoL model split never touch the same types. `App.svelte` shows the TFT
  view while `tft.phase != "none"`.
- `tft-coach` never fetches or reads the screen; all inputs are values (testable).
- `tft-choncc` depends on `tft-model` + `tft-coach` and implements
  `tft_coach::ModeHooks`. The app picks `ChonccHooks` when `queue == 1230`, else
  `StandardHooks`. Removing the mode = delete the crate + one line in `tft.rs`.

## Live advice (tft-coach)

Input per tick: `TftSnapshot` (vision), `Tracker` (history of snapshots), `TftSetData`,
`TftMeta` (queue-specific where MetaTFT has it), `&dyn ModeHooks`.

1. **Owned units** (`Tracker`): a shop slot that empties while gold drops by the unit's
   cost = bought; gold rising by a unit's sell value while the trait panel loses that
   unit's traits = sold (best effort). 3 copies → ★2, 9 → ★3. The trait panel counts
   (unique units per trait) correct the estimate: a trait count above what tracked units
   explain marks unknown units ("1 unknown Defender"). Carousel/orb/loot units are seen
   only through the trait panel.
2. **Comp fit** (automatic target): score every MetaTFT comp by units owned (★ weighted),
   traits active, components that build its carry items, augments taken, minus
   contested units; tie-break by avg placement. Top 3 kept; the best is the target.
   Switch target only when another comp beats it by a margin (stickiness, like LoL's
   `KEEP_MARGIN`).
3. **Shop**: per slot buy / skip with reason ("for Rengar comp", "makes ★2 Ahri",
   "contested by 2 boards").
4. **Econ**: level / roll / save from stage, gold, level, XP, HP, streak and the comp's
   `levelling` ("lvl 7" reroll vs "lvl 8"/"lvl 9" fast). Standard timings: lv 4 at 2-1,
   lv 5 at 2-5, lv 6 at 3-2, lv 7 at 4-1 (reroll: roll at 7) / lv 8 at 4-2, lv 9 at 5-1;
   interest at 10/20/30/40/50; roll down when HP < 40 or at the comp's spike stage.
5. **Items**: from held components, the best completed item for the target comp's carry
   builds (MetaTFT `builds`) and who should hold it; "slam now" when HP is falling or a
   stage's power spike is near.
6. **Augments**: rank the offered augments for the target comp and stage (MetaTFT stats
   if found; else comp `top_augments`; else no ranking).
7. **Contested**: units of the target comp in other players' trait panels seen this
   game (scouting), decayed after 2 rounds unseen.
8. **Calls** (spoken once per key): "Level to 8 now", "Roll down now", "Ahri is
   contested by 2", "Take <augment>".

## Choncc or Treat (tft-choncc)

- Rules: start with one 4- or 5-cost unit (same tier for all); loot orbs after each
  carousel; Choncc's Crown +2 / Chalice +1 team size (vision reads team size "7/9" from
  the board, so advice uses the real cap); comeback Golden Orb under 50 HP (radiant item +
  reforger + gold); some augments/encounters disabled.
- Comps by starting carry: MetaTFT comps (ranked data) re-scored for comps containing the
  starting unit, then my own Choncc games (placements by comp).
- Loot priorities: radiant > artifact > emblem/spatula for the target comp > components.
- Own stats kept apart from ranked in trends.

## Pre-game, post-game, trends

- Pre-game (lobby, queue known): top comps for the queue with avg placement, levelling
  style, carry items; Choncc: comps by starting carry.
- Post-game: from LCU TFT history (`placement`, units with items and ★, traits, level,
  last round): the comp it matches (nearest cluster), avg placement of that comp vs mine,
  carry items vs the comp's best builds, level at the end vs the comp's levelling.
- Trends: avg placement, top-4 rate, comps played and their results, per queue (Choncc
  apart); TFT LP via the existing ranked snapshot log (add `RANKED_TFT`).

## Testing

- Parsers: saved fixtures (Community Dragon Set 18 slice, MetaTFT comps/units/items,
  LCU TFT history) under `fixtures/tft/`.
- tft-coach: unit tests on synthetic snapshots (buy tracking, trait correction, fit,
  stickiness, econ timings, item from components, Choncc hooks).
- tft-vision: saved screenshots of the user's games (`%LOCALAPPDATA%\lol-coach\debug\tft\`)
  with expected readings; calibrated live.
- UI: mock states for TFT (in game, lobby, post-game) checked in the browser pane.
