# Live win probability, state-aware items and posture

Date: 2026-11-11. Status: approved 2026-11-11; everything but the fit is built (T0b done 2026-11-12, the crawl for the T3 fit is running), nothing shown yet (see Implementation status). Plan: `docs/superpowers/plans/2026-11-11-win-probability-plan.md`.

## Goal

Three things that share one number:

1. **Win probability** in game: my team's chance to win from what the scoreboard shows right now,
   with the factors behind it and what the next objective is worth.
2. **Item advice that reads the game state**: behind, even or ahead, and the enemy team's shape,
   from Coachless' per-situation WPA where signed in and from our own model otherwise.
3. **Posture**: a standing read of how to play the next minutes (press, hold, stall, look for a
   swing), built from the win probability, who the remaining game favours and the objectives up next.

Stopping condition: on held-out ranked games the model is calibrated (gates in "Testing"), the
in-game card and posture show in a real Rift game and replay sensibly over the recorded live
snapshots, item scores change in the documented cases only, and `cargo test --workspace`,
`npm --prefix web test` and `npm --prefix web run check` pass.

## Facts (checked 2026-11-11)

Coachless (site bundle and pages, signed out):

- There is **no live endpoint**. Win probability appears in two places: behind WPA (a model
  predicts the outcome from players, champions and game state "gold, levels, objectives and so
  on" at the moment of a purchase; WPA = win rate − average pre-buy win probability), and in the
  **Review** tool, post game: `GET /api/Review/GetMatchAnalysisForId/{..}/{..}` returns
  `gameStates[]` with `blueWinProbability` / `redWinProbability`, players, structures, towers,
  dragons, barons, and points of interest (`poiType` DEATH, KILL_TRADE, DEATH_FOR_OBJECTIVE,
  NEUTRAL_OBJECTIVE_LOST, OBJECTIVE_TRADE, TOWER_LOST, SPLIT, GOOD_PLAY, each with
  `deltaWinprob`). Review needs a linked account ("Upgrade to Link Account").
- **Items do change with the state**: `POST /api/ChampionWinprob/GetItemDetailed` (same body as
  `GetGlobalItemStatistics` plus `itemId`) returns an item's WPA and occurrence per situation in
  five groups: Damage (Physical / Balanced / Magic), Range (Low / Normal / High), Tankiness
  (Squishy / Normal / Tanky), CrowdControl (Low / Normal / High) and WinProbability (Behind /
  Even / Ahead), plus `averagePurchaseWinprob`. The site's enum runs 0–14 in that order. The
  reply's real shape is under "Coachless `GetItemDetailed`" below; `situationData` is the site's
  own view model, not a field of the reply.
- Also present, unused by us: `GetRecommendedBuilds`, `GetSkillUpSuggestion`, `GetItemUsers`.
- Their models are deep nets on 44M+ games and include player skill. We cannot match that and
  do not need to: a calibrated linear model on the scoreboard gets the bulk (gold, levels,
  objectives), and its coefficients are readable.

This repo:

- The Live Client API gives per player items, level, K/D/A, CS, dead / respawn timer, and events.
  `clients::live::parse` keeps kills, dragons, Baron, Herald, other `*Kill` and GameEnd, and
  **drops `TurretKilled`, `InhibKilled`, `InhibRespawned`** (they end in `Killed`). The fixture
  has `"TurretKilled": "Turret_T2_R_03_A"` (T1 = Order, T2 = Chaos; the turret named is the one
  that fell).
- Enemy gold is hidden; `gold::gold_diff` uses item gold and says so. That stays.
- `calls::readiness` is a hand-set score (item gold / 1000 + level diff + 0.7 per dead player).
  `items::Ctx::ahead` is my own KDA ≥ 2.5. `plan::game_plan` holds every champion's games and
  wins by game length (`Curve`), fetched for all ten.
- `%LOCALAPPDATA%\lol-coach\fixtures` holds 3,556 recorded `allgamedata-*.json` snapshots from
  real games, 30 s apart: real input for replaying the live path. The history cache keeps only
  compact `PastGame`s (no raw timelines), 30 games: not enough to fit a model.
- `RIOT_API_KEY` is not set. `docs/riot-api-key.md` describes getting a dev key.

Coachless `GetItemDetailed` (plan T0a, probed 2026-11-11 with the app's saved tokens, a free
account; reply saved as `fixtures/data/coachless-item-detailed-ahri-2503.json`: Ahri mid,
Emerald+, patch 16.21, item 2503 as first item):

- The reply is **one flat object**, no array and no `situationData`. Per situation it has a WPA
  in win-rate points (`delta…`) and the share of the item's purchases made in it, in percent
  (`…Occurrence`; the three of a group add up to 100):

  | group | WPA fields | share fields |
  |---|---|---|
  | Damage | `deltaAgainstPhysicalDamage`, `deltaAgainstBalancedDamage`, `deltaAgainstMagicDamage` | `physicalDamageOccurrence`, `balancedDamageOccurrence`, `magicDamageOccurrence` |
  | Range | `deltaWhenLowRange`, `deltaWhenBalancedRange`, `deltaWhenHighRange` | `lowRangeOccurrence`, `balancedRangeOccurrence`, `highRangeOccurrence` |
  | Tankiness | `deltaWhenSquishy`, `deltaWhenBalancedTankiness`, `deltaWhenTanky` | `squishyOccurrence`, `balancedTankinessOccurrence`, `tankyOccurrence` |
  | CrowdControl | `deltaWhenLowCC`, `deltaWhenNormalCC`, `deltaWhenHighCC` | `lowCCOccurrence`, `normalCCOccurrence`, `highCCOccurrence` |
  | WinProbability | `deltaWhenBehind`, `deltaWhenEven`, `deltaWhenAhead` | `behindOccurrence`, `evenOccurrence`, `aheadOccurrence` |

  Beside them: `wpaOverall`, `itemOccurrence` (games), `averageWinrate`,
  `averagePurchaseWinprob` (`wpaOverall` is the first minus the second: 52.889 − 51.777), and
  three lists, `componentDeltas[{itemId, count, delta}]`, `deltasByItemSlot[{slot, count, delta}]`
  and `deltasByElo[{tier, count, delta}]`. `deltasByElo` covers tiers 3–7 whatever `leagueTiers`
  asks for. Games per situation are not given: they are share × `itemOccurrence`.
- **Bucket boundaries are not published**: not in the reply, and the site's bundle holds only the
  labels (Physical / Mixed / Magic, Low / Normal / High, Squishy / Normal / Tanky, Behind / Even /
  Ahead) and the field mapping. The shares sit near a third each (here behind 26 %, even 39 %,
  ahead 34 %; damage 29 / 33 / 38), so the cut-offs look fixed and close to terciles. T6 has to
  choose its own thresholds for the live game and say so.
- **No account is needed**: the same body without a token answers 200 with the same numbers, on
  the newest patch (16.22) too, and so do `GetGlobalItemStatistics` and `GetRecommendedBuilds`.
  That differs from the notes at the top of `crates/data/src/coachless.rs` (2026-10-28: `[]`
  without a token, a free account one patch behind), which are out of date. What a free account
  does not get is a matchup: `matchupChampionIds` answers **403** on `GetItemDetailed` and on
  `GetGlobalItemStatistics`. Premium was not tested (no such account).
- **One item per call**: `itemId` is an Int32 (an array is a 400), an added `itemIds` is ignored.
  Without `itemSlots` and `itemType` the reply is 200 with every number 0, so a zero
  `itemOccurrence` means "no data", not an error.
- The other filters count: the same item as second legendary after item 3118 (`itemSlots: [2]`,
  `firstLegendaryId`) has 855 games and −1.7 WPA instead of 67,889 and +1.1; Master+ alone has
  4,559 games. An item with 13 games shows deltas of −24, so every situation term needs its
  shrink by games.
- The site reads Range's middle share from `balancedDamageOccurrence` (its bug); the reply has
  `balancedRangeOccurrence`.
- `GetRecommendedBuilds` takes `{commonFilters}` alone and returns `{role, builds[], spellDuos[]}`
  (saved as `fixtures/data/coachless-recommended-builds-ahri-middle.json`): four builds, each
  `{firstItemId, secondItemId, keystone, tier, winrate, occurrence, mostPopular, starterIds,
  itemBlocks[5]: {type 0–4, items[{itemId, wpa, occurrence, chooseOne, highlighted}]}, runes:
  {primaryTree, secondaryTree, selectedRunes, selectedShards, runeStats[], shardStats[][]}}`.
- The saved tokens are in `%LOCALAPPDATA%\lol-coach\coachless.json` (`settings::local_dir`), not
  under `%APPDATA%`.

Live events (plan T0c, over the 3,556 recordings, 2026-11-11):

- **81 games**: 48 Mayhem (`KIWI`, map 12; one with eight players, one a replay of the repo's
  fixture), 30 Rift (`CLASSIC`), 3 Swiftplay. Median recorded length 20 min, one under 5 min.
  **Only 2 have a `GameEnd`** (one Rift, one Mayhem, both wins): the game closes before the next
  30 s recording. These recordings can not measure calibration; that needs the Riot timelines (T3).
- Games must be told apart by the players' names and the clock. `championName` changes mid-game
  when a Viego holds a body (74 recordings), so grouping by champion splits them into 155.
- Event names and counts, read from each game's last recording: ChampionKill 7,525, Multikill
  1,385, TurretKilled 545, Ace 285, InhibKilled 135, DragonKill 121, HordeKill 93, GameStart 81,
  MinionsSpawning 81, FirstBlood 81, FirstBrick 79, InhibRespawned 43, BaronKill 25,
  HeraldKill 22, InhibRespawningSoon 4, GameEnd 2.
- **Structure names are not the ones assumed above.** Every real game has
  `Turret_TChaos_L1_P3_2254202041_0` and `Inhib_TOrder_L1_P1_196971597_0`: `T<Order|Chaos>` is
  the side that **lost** it (521 of 521 kills by a player are by the other team), `L0` bot, `L1`
  mid, `L2` top (outer turrets before 15:00 fall to BOTTOM, MIDDLE and TOP players in that order:
  10 of 11, 6 of 8, 18 of 19), `P3` outer, `P2` inner, `P1` inhibitor turret, `P4` and `P5` the
  nexus turrets (a lane's turrets fall in that order in every game; the nexus pair is named under
  `L1`). Howling Abyss has `L1` only, with `P3`, `P2`, `P4`, `P5`. The trailing number is fixed
  per structure and map. `Turret_T2_R_03_A` appears once, in a recording of the repo's own
  fixture; `Barracks_T1_C1` never.
- Samples (names removed): `{"EventName":"TurretKilled","KillerName":"…","TurretKilled":"Turret_TChaos_L0_P3_511845594_0","Assisters":[…]}`,
  `{"EventName":"InhibKilled","KillerName":"…","InhibKilled":"Inhib_TOrder_L1_P1_196971597_0","Assisters":[…]}`,
  `{"EventName":"InhibRespawned","InhibRespawned":"Inhib_TChaos_L1_P1_2432143522_0"}`,
  `{"EventName":"InhibRespawningSoon","InhibRespawningSoon":"…"}` (15 s before, 3 of 3),
  `{"EventName":"Ace","Acer":"…","AcingTeam":"ORDER"}`.
- **The killer can be a minion**: names like `Minion_T200L1S24N0151` took 23 of 545 turrets and
  1 of 135 inhibitors, so the side comes from the structure's name, never from the killer.
- An inhibitor comes back after 300 s on the Rift (5 of 5) and 250 s on Howling Abyss (38 of 38),
  and always falls after its lane's last turret (135 of 135).

Riot match-v5 (plan T0b, 2026-11-12: 20 ranked solo games, EUW, patch 16.22, crawled with a dev
key by `tools/winprob/crawl.py` and read by `features.py --check`):

- Match: `info.gameVersion` "16.22.834.8910", `gameDuration` in seconds, `endOfGameResult`
  "GameComplete", `teams[].win`; per participant `participantId` 1–10, `teamId` 100 / 200,
  `championId`, `item0`…`item6`, `gameEndedInEarlySurrender`, `teamEarlySurrendered`,
  `gameEndedInSurrender`, and **`roleBoundItem`**: a slot of its own that holds a role quest
  reward (ids 12xx, cost 0) or, for 40 of 200 players, their boots.
- Timeline: `frameInterval` 60000; frames 60 s apart plus one shorter last frame at the game's
  end (within 60 s of `gameDuration` in 20 of 20); `participantFrames` keyed "1"…"10" with
  `totalGold`, `currentGold`, `level`, `minionsKilled`, `jungleMinionsKilled`, `xp`, `position`
  and stat blocks. `GAME_END.winningTeam` agrees with `teams[].win` (20 of 20).
- Event types in the 20 games: ITEM_PURCHASED 4,091, ITEM_DESTROYED 3,859, WARD_PLACED 2,815,
  SKILL_LEVEL_UP 2,734, LEVEL_UP 2,465, CHAMPION_KILL 926, TURRET_PLATE_DESTROYED 920,
  WARD_KILL 646, BUILDING_KILL 199, ELITE_MONSTER_KILL 154, ITEM_SOLD 137, ITEM_UNDO 131,
  CHAMPION_SPECIAL_KILL 125, DRAGON_SOUL_GIVEN 23, OBJECTIVE_BOUNTY_PRESTART 21, PAUSE_END 20,
  GAME_END 20, OBJECTIVE_BOUNTY_FINISH 5.
- `BUILDING_KILL`: `buildingType` TOWER_BUILDING / INHIBITOR_BUILDING, `towerType` OUTER_ /
  INNER_ / BASE_ / NEXUS_TURRET, `laneType`; `teamId` is the side that lost it (never the
  killer's team, 193 of 193); `killerId` 0 when a minion took it. A winner can lose a nexus
  turret (1 of 22).
- `ELITE_MONSTER_KILL`: `monsterType` DRAGON / HORDE / RIFTHERALD / BARON_NASHOR, `killerTeamId`
  is the killer's team (151 of 151), `killerId` can be 0; dragon subtypes FIRE_, WATER_, EARTH_,
  AIR_, HEXTECH_, CHEMTECH_DRAGON. One HORDE event per grub. No Elder and no Atakhan in these 20.
- **`DRAGON_SOUL_GIVEN` is sent twice**: with `teamId` 0 when the soul's kind is announced after
  the second dragon (18 of 23), and with the side's id when it takes its fourth (5 of 23).
- `CHAMPION_KILL`: `killerId` 0 for an execution; `victimId`, `assistingParticipantIds`.
- **Item events do not add up to the inventory.** Each support's World Atlas is an
  `ITEM_PURCHASED` by participant 0 at 0:00; its later stages and its upgrade arrive with no
  purchase, only an `ITEM_DESTROYED` of the stage before; an item bought whole destroys
  components the player never bought; rune and quest items come and go without a purchase; 4 of
  131 `ITEM_UNDO` have neither `beforeId` nor `afterId`. Rebuilt with rules for these
  (`features.Inventories`), 191 of 200 players end with the item gold of their final items and a
  team's item gold is off by 0.46 % on average, 6.2 % at worst. Gold spent
  (`totalGold − currentGold`) is 7.6 % above held item gold on average at the end of a game.
- Crawl speed on a dev key: about 1,000 stored games an hour (half the match ids of ladder
  players are from another patch and cost a request each).

From the first 1,221 games of the full crawl (same day, `features.py --check --limit 500`):

- An Elder dragon is `monsterType` DRAGON with `monsterSubType` ELDER_DRAGON (59 events). No
  Atakhan event in any game. `killerTeamId` is **300** for a monster no side killed: a Rift
  Herald (88 of 1,122) and a Void Grub (8 of 3,609).
- A remake has `gameEndedInEarlySurrender` and lasts 66 to 134 s (9 of 500); two more games were
  under 5 minutes. `CHAMPION_TRANSFORM` exists (16 events, ignored).
- **Viego**: each possession sends `ITEM_DESTROYED` for all he holds, then for all the body
  held, and nothing when his own items are back. Counted as written he ends up to 11,300 g
  short; a destroyed item of his counts only when he bought something at that timestamp.
- A Tear item bought with the Tear full is purchased and destroyed at one timestamp and is its
  upgrade from then on.
- With these rules, on 489 kept games: 4,777 of 4,890 players end with the item gold of their
  final items; a team's item gold is off by 0.11 % on average, 4.2 % at worst. Most of the rest
  are mid laners with the quest reward (`roleBoundItem` 1206), whose boots change with no event
  that says to what. Nexus turrets fall on the losing side in 765 of 786.

Policy (see `docs/ideas.md` section 1): every input is on the scoreboard or announced; nothing
estimates enemy gold or cooldowns. A win probability is analysis of known information. Posture is
worded as a read plus options, as the game-plan card is ("facts and comparisons, never orders");
Riot lists "apps that dictate player decisions" as unapproved.

## Design

### 1. Model (`crates/coach/src/winprob.rs`, pure; coefficients in `crates/coach/winprob.toml`)

Logistic regression with time-varying coefficients:

```text
z = Σ_k β_k(t) · x_k        p = 1 / (1 + e^−z)
β_k(t): linear between knots at 0, 10, 15, 20, 25, 30, 40 min (flat after 40)
```

Features, all mine − theirs (antisymmetric, so one row per sample and no intercept besides `side`):

| key | value | live source |
|---|---|---|
| `gold` | ln((I_mine + 3000) / (I_theirs + 3000)), I = team item gold | `common::inventory(..).gold` |
| `level` | mean level difference | `Player::level` |
| `cs` | team CS difference / 100 | `Player::cs` |
| `kills` | (K_mine − K_theirs) / sqrt(K_mine + K_theirs + 4) | `Player::kills` |
| `towers` | towers taken, difference | new `GameEvent::Tower` |
| `inhibs` | their inhibitors down now − ours | new `GameEvent::Inhib` + respawn |
| `drakes` | dragons, each side capped at 3, difference | `GameEvent::Dragon` |
| `soul` | ±1 once a side has the soul | same |
| `elder`, `baron` | ±1 while the buff lasts | event time + duration |
| `grubs` | grub difference / 3 | `GameEvent::Objective` |
| `dead` | Σ over their dead min(respawn, 45)/45 − same for ours | `Player::dead`, `respawn_in` |
| `outlook` | Σ_mine (wr_c(≥t) − 50) − Σ_theirs, win-rate points | `plan::Curve` |
| `side` | +1 on blue, −1 on red | `Player::team` |

`wr_c(≥t)`: champion c's win rate in games that last at least t minutes (wins and games summed
over the length buckets from t on, shrunk with `common::shrink`). This is the draft prior and the
scaling read in one feature: early it is close to the champions' plain win rates, late it favours
scalers. Missing curves contribute 0 and set `rough`.

`cs` and `kills` are kept as separate inputs because they carry income not yet turned into
items; nothing is displayed as "enemy gold". The fit drops any feature whose sign is unstable
across folds.

Output (`model-live`, in `GameAdvice.win: Option<WinProb>`):

```text
WinProb { p, base, trend, factors: Vec<WinFactor>, stakes: Vec<Stake>, outlook: Option<Outlook>, rough, source }
WinFactor { key, label, points }      // p(z) − p(z − β_k x_k), in points, largest first, |points| ≥ 1
Stake { name, swing, note }           // p(I take it) − p(they take it), in points
Outlook { now, later, later_min }     // outlook feature now and 10 min on
```

- `p` is the full model; `base` leaves `dead` out (the standing state, used for item and posture
  decisions so one death does not flip them). `trend` is the change of `base` over 120 s, from the
  samples already kept in `GoldHistory` (`GoldSample` gains `win: Option<f32>`).
- **Stakes**: for the next Dragon (with the soul when a side has 3), Baron, Elder, the next tower
  and an inhibitor, re-evaluate the state with the objective on either side. Also "my death now":
  the state with me dead for `fights::respawn(level, t)`. Sorted by swing.
- Modes: Rift (also Swiftplay, marked `rough`) from the `rift` table; ARAM from an `aram` table
  (gold, level, kills, towers, inhibs, dead, outlook); Mayhem uses the ARAM table and is always
  `rough` with "ARAM model, augments not counted", as the game-plan card does.
- `winprob.toml` carries `patch`, `games` and the held-out metrics. With `games = 0` (no fit yet)
  `WinProb` is None and everything below falls back to today's behaviour.

### 2. Fitting (`tools/winprob/`, offline, Python as `tools/tft-render`)

- `crawl.py`: Riot match-v5 with `RIOT_API_KEY`. Ladder entries (Emerald+, the highest leagues
  first; see "Third pass") → puuids → ranked solo match ids → match + timeline, raw JSON gzipped under
  `%LOCALAPPDATA%\lol-coach\dev\winprob\`. Resumable, obeys the 100 requests / 2 min limit
  (about 1,500 games an hour); target 10k games on the current patch, ARAM (queue 450) 5k.
- `features.py`: one row per minute frame per game from blue's side, the features above rebuilt
  from the timeline: held items from `ITEM_PURCHASED` / `ITEM_SOLD` / `ITEM_DESTROYED` /
  `ITEM_UNDO` with Data Dragon costs (same meaning as live item gold), buildings and monsters from
  `BUILDING_KILL` / `ELITE_MONSTER_KILL` / `DRAGON_SOUL_GIVEN`, who is dead from kills and the
  respawn formula in `fights.rs`, `outlook` from champion-by-length tables built on the training
  split. Remakes and early surrenders are left out.
- `fit.py`: L2 logistic regression on the hat-function basis (feature × knot), numpy only,
  games weighted equally, split by game id (80 / 20), five folds for sign stability. Writes
  `winprob.toml` and `fixtures/coach/winprob_cases.json` (200 held-out states with the Python
  probability) for a Rust parity test.

### 3. Items (`crates/coach/src/items.rs`, `crates/data/src/coachless.rs`)

- `Standing { Behind, Even, Ahead }` from `base` (≤ 0.42, ≥ 0.58). `Ctx::ahead` becomes
  `Standing::Ahead` and my item gold at or above my team's average; the KDA rule stays as the
  fallback without a model.
- **Coachless situations** (signed in): `coachless::item_situations` calls `GetItemDetailed` for
  the current slot's candidates (Buy next, the alternatives, the next two path items: at most
  six requests per slot, cached 12 h on disk, fetched through `state::ensure`). The scorer adds
  `Σ_group shrink(wpa_situation − wpa_overall)` for the situations the live game is in:
  WinProbability from `Standing`, Damage from the threat-weighted enemy magic share, Tankiness
  and CrowdControl from the foes' traits and items, Range from enemy attack ranges. Where an item
  has situation data, the hand-set `answers · needs` term counts half (it is the same question
  answered with less evidence). The reason line names it: "better from behind (Coachless)".
- **Without Coachless**: behind, defensive needs × 1.15 and a tempo term that lowers items whose
  ETA at my tracked income is far off (a 3,500 g item at 300 g/min is not a comeback buy); ahead,
  defensive needs × 0.9. Small constants in `consts.rs`, each below `KEEP_MARGIN` on its own.

### 4. Posture (`crates/coach/src/posture.rs`, pure; `PostureMemo` kept by the orchestrator)

```text
Posture { stance: Stance, headline, why: Vec<String>, options: Vec<String> }
Stance: Press | Hold | Even | Stall | Swing
```

| state (`base`) | remaining game favours | stance | read |
|---|---|---|---|
| ahead (≥ 0.60) | them, or nobody | Press | the lead is worth most now: objectives on spawn, deny, end |
| ahead | us | Hold | time is on your side: take what is free, no coin flips |
| 0.40–0.60 | either | Even | the next objective decides: its swing is named |
| behind (≤ 0.40) | us | Stall | trade, clear waves, give what costs least; the game comes to you |
| behind | them | Swing | time is against you: picks on shutdowns, fights with numbers at the objective |

"Favours" is `Outlook.later − Outlook.now` beyond `PLAN_EVEN`. The stance holds for at least 45 s
and flips only 3 points past a threshold. `why` lists the top factors and the outlook line;
`options` are two or three choices built from live facts: the highest stake and its timer, named
shutdowns (`Bounty`), who is dead and for how long, my item ETA. A stance change adds one `Call`
(spoken only with the new `posture_calls` setting, off by default).

`calls::readiness` is rebased on the model: Fight when `p − base ≥ 6` points or `base ≥ 0.60`,
Avoid on the mirror, with the factors as its reasons. Take-objective calls gain the swing
("Baron is up: 2 dead. ±11").

### 5. Review

- `MatchAnalysis.gold[i]` gains `win: Option<f32>`: from the live samples for games played with
  the app, else from the LCU timeline frames (`MatchDetail`; item gold is replaced by
  `total_gold − current_gold` there).
- `MatchAnalysis.swings: Vec<Swing { time, delta, kind, text }>`: the five largest minute-to-minute
  changes, each tied to what happened in that minute (a fight lost 1 for 3, Baron given, my death).
- Macro coach, later: games lost from ≥ 65 % and won from ≤ 35 % as a pattern (needs the peak and
  low stored on `PastGame`).

### 6. UI

- `web/src/components/ingame/WinProb.svelte`: two-sided bar with the percentage, trend arrow,
  sparkline from the gold history, factor chips ("gold +9", "dragons −4"), stakes rows. `rough`
  shows a tilde and the reason.
- `Posture.svelte` next to Calls: stance, headline, why, options.
- `GoldChart.svelte` and the review's gold chart overlay the win line; the review lists swings.
- Settings: `win_prob` (on), `posture_calls` (off). Overlay: nothing in v1.

## Errors and gaps

- No fit shipped, or the mode has no table: no card, no posture, items as today.
- Positions are not in the model (the minimap reader is not reliable enough to train against).
  Fog, wave states and cooldowns are invisible to it; the card says "from the scoreboard".
- Player skill is not known (no look-ups); the number is for an average lobby of the fitted tier.
- A new patch shifts item costs and objectives: the source line names the fit's patch; refit when
  the gates fail on fresh games.
- Coachless signed out, lapsed, or the item has too few games in a situation: that term is 0.

## Testing

- Model gates on held-out games, written into `winprob.toml`: expected calibration error ≤ 2
  points overall and ≤ 3 in every 5-minute bucket from 5:00; log loss below a gold-only model;
  accuracy at 10 / 15 / 20 / 25 min reported.
- Rust parity: `winprob_cases.json` reproduces to 1e-4.
- Unit tests with an inline coefficient table: symmetry (swapping teams gives 1 − p), each
  feature moves p the right way, stakes ordering (Baron above a first dragon late), `base`
  ignores deaths, knot interpolation, `rough` cases, None with `games = 0`.
- Live parser: `Tower` and `Inhib` events from the fixture; inhibitor respawn.
- `cargo run -p lol-coach --example winprob_replay`: p over every recorded game in the local
  fixtures directory, with the result where a GameEnd was recorded.
- Posture: one test per table row, hysteresis, options built from stakes and bounties.
- Items: an item with better "behind" WPA rises when behind and not when even; no situation data
  leaves today's order unchanged in the existing item tests.
- UI: mock states `ingame rift` (ahead), a behind state and a rough Mayhem state, checked in the
  browser.

## Decisions (approved with the spec)

1. Training data from the Riot API with a dev key (needs the key set once). Alternative: a public
   timeline dataset from an older patch, weaker.
2. Posture wording is a read plus options, not orders.
3. The percentage is shown as a number (not a band).

## Implementation status (2026-11-11)

Built: T0a, T0c, T1, T2, T4, T5, T6, T7, T8, the code of T3 and the rules of T9. Not done: T0b
(one real Riot timeline), running T3 (the crawl and the fit) and T9's ARAM fit; all three need
`RIOT_API_KEY`. Coachless Premium was not needed and not tested.

**Nothing is shown to the user yet**: `winprob.toml` ships with `games = 0`, so in the app
`GameAdvice.win` and `posture` are None, readiness and item scoring work as before, no Coachless
situation is fetched, and the review has no win line. Everything below ran against the hand-set
table in tests and mock states only. The day `fit.py` writes a table with `games > 0`, all of it
turns on at once; the `win_prob` setting turns it off again.

Checks at the end of the second pass: `cargo test --workspace -q` 792 passed, 0 failed, 16 ignored;
`npm --prefix web test` 9 passed; `npm --prefix web run check` 0 errors; `tools/winprob` 21 tests
passed; the replay still runs over the 81 recorded games.

The sections "Built", "Replay" and "Deviations" that follow are the first pass (T0a, T0c, T1, T2,
T4); "Second pass" after them covers the rest.

Built:

- T0a, T0c: the two "Facts" blocks above; fixtures `coachless-item-detailed-ahri-2503.json` and
  `coachless-recommended-builds-ahri-middle.json`. The probe example was deleted after use.
- T1: `GameEvent::Tower { time, team, lane, tier }` and `GameEvent::Inhib { time, team, lane,
  respawned }` (`team` lost it); `live::structure` reads the side, lane and tier from the name;
  tests on `live_allgamedata.json` and on `fixtures/clients/live_structure_events.json` (events
  from recorded games, player names swapped for the fixture's).
- T2: `crates/coach/src/winprob.rs` (`Model`, `Table`, `State::from_snapshot`,
  `State::from_frame`, `beta`, `eval`, `win_prob`), `crates/coach/winprob.toml` (no fit), types
  `WinProb`, `WinFactor`, `Stake`, `Outlook` and `GoldSample.win` in `model-live`, constants
  `BARON_BUFF` 180 s, `ELDER_BUFF` 150 s (wiki, 26.22) and `INHIB_RESPAWN` 300 s / 250 s (the
  recordings) in `consts.rs`. 17 tests in `crates/coach/tests/winprob.rs` cover the list in
  "Testing"; the parity test passes vacuously until `winprob_cases.json` exists.
- T4: `GameInput.curves`, `income` (carried, unused until T6) and `winprob`; `game_advice`
  computes `win` first and `gold_diff` stores `base` in the sample it pushes; the orchestrator
  builds the ten curves once per tick (`lengths`) for both the coach and the game-plan card;
  the smoke example prints the win line; `winprob_replay` runs over the recordings.

Replay (`cargo run -p lol-coach --example winprob_replay`, hand-set table, no curves): 3,556
recordings, 81 games, none unparsed, no panic. Stomps run to one side and stay there, close
games sit near the middle. Result known for 42 games, 2 from `GameEnd` and 40 inferred from a
side having lost both nexus turrets in the last recording; over their 1,800 snapshots:

| p | snapshots | mean p | won |
|---|---|---|---|
| 0–10 | 73 | 5 % | 4 % |
| 10–20 | 87 | 16 % | 21 % |
| 20–30 | 123 | 25 % | 32 % |
| 30–40 | 174 | 35 % | 39 % |
| 40–50 | 222 | 46 % | 54 % |
| 50–60 | 317 | 54 % | 59 % |
| 60–70 | 240 | 66 % | 72 % |
| 70–80 | 181 | 75 % | 75 % |
| 80–90 | 147 | 85 % | 87 % |
| 90–100 | 236 | 96 % | 92 % |

The last snapshot is on the winner's side in 40 of 42. This is a smoke test, not a calibration:
the coefficients are made up, the games are one player's and more often won than lost (24 of 42), snapshots of
one game are not independent, and the games with a known result are the ones recorded to the end.

Deviations:

- Structure names: the plan expected `Turret_T2_R_03_A` and `Barracks_T1_C1`. Real games send
  `Turret_TChaos_L1_P3_…` and `Inhib_TOrder_L1_P1_…` (T0c), which the parser reads in full. The
  old turret name is still read for its side and its side lane (`tier` 0: the name does not say
  it for certain); `Barracks_…` was never seen and is skipped.
- Inhibitors in a live game are down from `InhibKilled` until `InhibRespawned`; no timer is
  involved. Only `State::from_frame` times them (the LCU timeline has no respawn event).
- The test table is a file, `fixtures/coach/winprob_handset.toml` (marked hand-set, same schema
  as `winprob.toml`), because the replay needs the same table as the tests. The tests read it
  with `include_str!`; the two interpolation tests use tables written inline.
- `GameInput.winprob: Option<&Model>` is added beside `curves` and `income` (None reads the
  built-in file), so a test can run `game_advice` with a fitted-looking table.
- `eval(knots, table, state, minute)` takes the knots too: they belong to the file, not to a
  mode's table. `beta` is public for the parity test.
- `WinProb.trend` is `Option<f32>` in points, taken from the sample closest to 120 s back and at
  most 60 s off (the history thins out to 40 s steps and more in long games). `Stake.note` is
  optional. Why a number is `rough` is said in `source`; the type has no separate field for it.
- Stakes listed: Dragon while nobody has the soul (noted "your soul" / "their soul" at three),
  Elder after it, Baron from 17:00, Tower always, Inhibitor once an inhibitor turret has fallen
  (Howling Abyss: the second turret), "Your death" on the Rift only (`fights::respawn` is the
  Rift's formula). ARAM and Mayhem therefore get towers and inhibitors alone, as T9 wants.
- `rough` is also set when a champion has no curve only if the mode's table uses `outlook`.
- `State::from_frame` has no `side` and no `outlook` (0): `MatchDetail` does not say which side
  I was on and the review has no curves. T8 should add both if the fit keeps those features.
- The replay does not fetch the champions' curves (about 150 lolalytics pages for a hand-set
  table); the outlook counts 0 there. Worth adding when a real fit is replayed.
- The replay's "result" also counts a side that lost both nexus turrets, marked `~`, since only
  2 games have a `GameEnd`.
- `web/src/mock`: `win: null` added to the two in-game states and the gold samples, because the
  generated types now require the field.

Left for a real game: nothing is visible until a fit exists. With a fit, check that `Tower` and
`Inhib` arrive as parsed here in a live Rift game and that `trend` fills in after two minutes.

Found on the way, not changed:

- `live::parse` takes a Viego's `championName` as it comes, so he is another champion while he
  holds a body.

### Second pass (2026-11-11): T3 code, T5, T6, T7, T8, T9 rules

Built:

- T5 posture: `Posture`, `Stance` in `model-live`, `GameAdvice.posture`; `crates/coach/src/posture.rs`
  (`PostureMemo::decide`, constants `POSTURE_*`), kept in the orchestrator's `Memo`;
  `calls::readiness` reads `win` (Fight when `p − base ≥ 6` points or `base ≥ 0.60`, Avoid on the
  mirror) and keeps the old score rule without it; take calls end with the stake's swing; a stance
  change adds a call `stance-<stance>-<n>` for 45 s. Settings `win_prob` (on) and `posture_calls`
  (off) on `Settings` in `crates/model`. Tests: `tests/posture.rs` (10), two in `tests/calls.rs`.
- T6a: `Standing` from `win.base` (0.42 / 0.58), `Ctx::ahead` from it and my item gold against my
  team's average (KDA rule without a fit), defensive needs × 1.15 behind and × 0.9 ahead, a tempo
  term behind from my income (at most 0.015). T6b: `coachless::item_situations` (the flat reply,
  one item per call, the slot's own body, a matchup only with Premium), `Situation`,
  `ItemSituations`, `BuildSlot` in `model-static`, `crates/coach/src/situations.rs` (the live
  game's situation per group, the shrunk term, the reason "better from behind (Coachless)"),
  `Shared::situations` filled through `state::ensure` for last tick's candidates (at most six).
  Tests: `tests/standing.rs` (7), two in `coachless.rs`. The notes at the top of `coachless.rs`
  now state what the probe found.
- T7: `WinProb.svelte`, `Posture.svelte` in `InGame.svelte` under the calls; the win line on the
  in-game gold sparkline (`GoldDiff.svelte`); `web/src/lib/winprob.ts` with tests; the two
  settings toggles; three mock states, `ingame ahead` (Press, 74 %), `ingame behind` (Stall, 35 %)
  and `ingame mayhem rough` (Even, ~58 %), checked in the browser at 1280, 1920 and 700 wide.
- T8: `GoldPoint.win`, `Swing`, `MatchAnalysis.swings`, `Recap.win_curve` (live samples of `base`
  from `GameLog`, every 20 s); `analysis::win_curve` and `swings` (reusing `fights::fights` and
  the timeline marks; a buff running out and an inhibitor coming back also count as causes); the
  dotted win line on the review's `GoldChart.svelte` and `Swings.svelte`; the mock analysis
  carries a curve and five swings computed from the recorded flex game. `State::from_frame` now
  has `side` (participants 1 to 5 are blue).
- T9 rules: on Howling Abyss the state zeroes `cs`, the dragon, Baron, Elder and grub features and
  `side`, whatever the table holds; posture there has no objective options. The ARAM table itself
  needs the fit.
- T3 code, `tools/winprob/`: `riot_format.py` (every assumption about the Riot API in one file),
  `crawl.py`, `features.py` (the fourteen features as `State::features` computes them, and
  `--check`, which runs T0b's checks on real data), `fit.py`, `README.md`, `test_winprob.py`
  (21 tests: keys and constants read from the Rust sources and compared, a hand-built timeline,
  the crawler against a fake API, and a synthetic fit). **It has never been run against the Riot
  API.** On 20,000 simulated games with known coefficients the fit recovers every sign and each
  size within 25 %, held-out ECE 0.7 points against 2.9 for gold alone, and the Rust replay reads
  the table it writes. `fit.py` refuses to write synthetic rows into the repo.

Deviations and choices made in the second pass:

- Coachless situations count only while a fit exists (`win` is Some), and are not fetched without
  one: "no fit: items as today". Four of the five groups do not depend on the model and could run
  today; that is a decision for the user, a one-line gate in `game_advice` and the orchestrator.
- The situation cut-offs are ours, not Coachless': terciles over 238 enemy teams from the recorded
  Rift games (magic share 0.35 / 0.47, mean base range 305 / 360, tankiness 0.34 / 0.465, CC
  0.445 / 0.515). A situation counts from 300 games and is shrunk by g / (g + 1000).
- The situation cache key holds the first two built items and the slot, since the request does.
  One item's data is used wherever that item is scored, also in a later slot.
- The defensive-needs scaling can exceed `KEEP_MARGIN` for a very defensive item against several
  maxed needs (about 0.028); the tempo term can not.
- The stance call is always among the calls; the web layer speaks it only with `posture_calls`.
- Posture: hysteresis is on the `base` thresholds only; behind with no outlook reads Swing, ahead
  with none Press; when `base ≥ 0.60` and the dead say otherwise, the dead decide readiness.
- UI: the win line went on `GoldDiff.svelte` in game and on `GoldChart.svelte` in the review. The
  existing `ingame rift` mock is a behind game and stays as the "no data" check; the ahead state
  is new. The app has a dark theme only, so there was no light variant to check.
- `WinProb` has no field for why a number is rough; a Rift game that is rough for a missing curve
  shows only the source line.
- Review: swings need at least 1 point; a swing's time is its main cause's; both curves are
  `base`. The review's source is a fixed sentence and does not name the fit's patch. Mayhem reads
  the ARAM table there without a rough mark.
- Fit gates: a 5-minute bucket with fewer than 1,500 held-out games is reported, not gated (at
  1,000 games a perfectly calibrated model measures about 3 points of ECE). The overall 2-point
  gate is tight with 2,000 held-out games; 20k crawled games would be safer than 10k. The sign
  check fits five disjoint fifths. The L2 penalty is applied on each feature's scale.
  Surrenders from 15:00 are kept; remakes and games under 5 minutes are dropped.

Open, to settle before or with the first real fit:

- **Outlook scale.** Live, the outlook comes from lolalytics curves (tens of thousands of games
  per champion, hardly shrunk). The fit builds its curves from the crawled games (a few hundred
  per champion, shrunk hard by the same k = 400). The fitted coefficient will then overstate the
  outlook in a live game. Either fit on lolalytics curves or shrink both to a like sample size.
- **Review gold.** `State::from_frame` uses gold spent, the model is fitted on held item gold. On
  the recorded flex game three of the five swings are buy timing ("Buys and levels"); with total
  gold earned every swing lands on a fight or an objective, but a side holding gold is overstated.
- `ddragon::parse_items` applies item transforms in one pass, so Bounty of Worlds (a transform of
  a transform) is not loaded and counts 0 gold live; the fit mirrors this.
- Live `dead` uses the real death timer, the fit the formula at the level before the kill.
- With the hand-set table the live fixture reads Ahead and Mejai's Soulstealer becomes Buy next on
  its raw win rate; snowball items need a look once a real fit can call a game ahead.
- `champ_stats` and `matchup_runes` still send `matchupChampionIds`, which a free Coachless
  account gets 403 for. `fixtures/clients/match_timeline.json` is hand-built, not a recording.

With a key, in order (details in `tools/winprob/README.md`): crawl 20 games, run
`features.py --check` (this is T0b; fix `riot_format.py` and the Facts above if anything
differs), crawl the full set, `features.py`, `fit.py`, then `cargo test -p coach --test winprob`
and the replay. Then a real Rift game: the card, the posture, the stance call, the trend after
two minutes, and a past game in the review.

### Third pass (2026-11-12): T0b, the crawl, a nightly job

- T0b is done on real replies (Facts, "Riot match-v5"); `tools/winprob` follows them: support
  items bought by participant 0, items that change with no purchase, `roleBoundItem`, the soul
  event sent twice, killer team 300, Viego's possessions, a Tear item finished as it is bought.
- **Whose games** (decided by the user, 2026-11-12: any Emerald+, the higher the better).
  `crawl.py` takes Challenger, Grandmaster and Master players first, then Diamond and Emerald
  divisions page by page in turn. The apex leagues alone fill 20,000 games, so the fit is in
  practice on Master and above; `--no-apex` starts at Diamond. The user plays at Emerald and
  Diamond: if the number reads too sure of a lead in their games, this is the first thing to
  look at.
- The crawl for the first fit runs on a dev key (EUW, 16.22, into `dev\winprob\16.22\`): 2,002
  games of Challenger players and 94 of Diamond I players from a short run without the apex
  leagues. A trial fit on the first 1,055 training games ran end to end and failed the
  calibration gate as expected at that size (ECE 4.13 against 2.0; log loss 0.584 against 0.600
  for gold alone; Elder, grubs, outlook and side dropped for unstable signs). Nothing is shipped
  from it.
- `tools/winprob/nightly.py` is one run for a scheduler (cron, a systemd timer, Task Scheduler;
  README "Every night"): it crawls the newest patch for two hours into a directory per patch,
  refits at 10,000 games and every 2,500 more into that directory (never the repo), and removes
  the raw games of all but the two newest patches. `crawl.py` got `--hours`, `--key-file`, the
  key from `riot-api-key.txt`, and stops a player's list at the first match of an older patch.
  Tried for real on Windows (36 games, the fit step run and refused); not yet run on Linux. It
  needs a personal API key: a dev key lasts a day.
- A table fitted on another machine reaches the app by download (decided 2026-11-12).
  `nightly.py --publish DIR` copies the fitted table to a directory and, in a git checkout,
  commits and pushes it; the app (`src-tauri/src/winprob.rs`) reads `winprob.toml` from the
  repo's `winprob-data` branch at start and every 6 hours (`LOL_COACH_WINPROB_URL` names another
  address), through the disk cache. It replaces the built-in table when `Model::fault` finds
  nothing wrong with it and `Model::newer_than` says it is a later patch or the same patch on
  more games; otherwise the built-in table stays. The branch exists and is empty (made 2026-11-12), so
  nothing is downloaded until a fit passes and is published. A table can change during a game; the trend and
  the review curve then mix two tables for that game.

## Sources

- Coachless WPA method: https://coachless.gg/wpa · Review tool: https://coachless.gg/review
- Riot policies: https://developer.riotgames.com/docs/lol
