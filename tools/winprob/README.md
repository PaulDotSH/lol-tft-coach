# winprob

Fits the in-game win probability (`crates/coach/src/winprob.rs`, coefficients in
`crates/coach/winprob.toml`) from Riot match-v5 timelines. Spec:
`docs/superpowers/specs/2026-11-11-win-probability-design.md` (section 2 and "Testing"); plan task T3.

**Status: written and tested offline, never run against the Riot API.** No `RIOT_API_KEY` was
available, so the crawler has not made a request and the timeline format is taken from Riot's
documentation, not from a reply (plan task T0b is undone). Read "Unverified until a key exists"
before trusting a fit.

Python is the system 3.12 with `requests numpy`, run as `py -3.12 -I`. Dev data lives outside the
repo in `%LOCALAPPDATA%\lol-coach\dev\winprob\`: `raw\` (gzipped match and timeline JSON),
`crawl_state.json`, `item-<version>.json` (Data Dragon) and `rows.npz`.

| file | does |
|---|---|
| `riot_format.py` | every endpoint, field name, event type and enum value the tools assume, in one place |
| `crawl.py` | ladder players → match ids → match + timeline, resumable, rate limited |
| `features.py` | rows (one per minute frame, blue's side) with the Rust features; `--check` runs the T0b checks |
| `fit.py` | the fit, the report, the gates; writes the TOML and the parity cases only when the gates pass |
| `test_winprob.py` | offline tests (below) |

## Run, once a key exists

```
setx RIOT_API_KEY RGAPI-...        (docs/riot-api-key.md; open a new terminal)
tasklist | findstr /i "League"     (nothing long while a game runs)

py -3.12 -I tools/winprob/crawl.py --games 20 --out %LOCALAPPDATA%\lol-coach\dev\winprob\probe
py -3.12 -I tools/winprob/features.py --check --raw %LOCALAPPDATA%\lol-coach\dev\winprob\probe\raw --limit 20
```

The check must end with every player's item gold rebuilt and every agreement line "N of N"; it
exits 1 otherwise, and any `FormatError` names the match, frame and field. Fix
`riot_format.py` (and the spec's "Facts") from what the real replies show, then:

```
py -3.12 -I tools/winprob/crawl.py --games 10000            (about 7 h on a dev key; rerun it after a stop or a new key)
py -3.12 -I tools/winprob/features.py --check --limit 500
py -3.12 -I tools/winprob/features.py                       (→ rows.npz)
py -3.12 -I tools/winprob/fit.py                            (→ crates/coach/winprob.toml, fixtures/coach/winprob_cases.json)
cargo test -p coach -q --test winprob                       (parity: the cases reproduce in Rust to 1e-4)
cargo run -p lol-coach --example winprob_replay             (the fit over the recorded live games)
```

`crawl.py` options: `--platform euw1 --region europe` (routing), `--queue 420` (450 for ARAM),
`--patch 16.22` (default: newest on Data Dragon; matches on other patches are not stored),
`--days 14`, `--out DIR`. A dev key expires after 24 h: the crawler saves its state and stops on
401 / 403. `features.py --mode aram` and `fit.py` on its rows write the `[aram]` table and keep
an existing `[rift]` one. `fit.py --out X --cases Y` writes elsewhere; rows marked synthetic are
refused for any path inside the repo.

## What the tools compute

- Features, keys and order are `State::features` (`KEYS` in `winprob.rs`), from blue's side
  (`side` = +1 on every row, so its coefficient is blue's edge): item gold held, rebuilt from the
  item events with Data Dragon costs as the app counts them (`ddragon::parse_items` shop filter
  and transforms, consumables 0), mean level, minions + jungle CS, kills, towers taken,
  inhibitors down (timed: 300 s on the Rift, 250 s on Howling Abyss), elemental dragons capped at
  3 plus the soul, Elder and Baron while the buff lasts, grubs, the dead from kills and
  `fights::respawn` with the victim's level at the frame before the kill (capped at 45 s), and the
  outlook from champion-by-length tables (lolalytics' buckets 0, 15, … 40 min) built on the
  training split and shrunk as `winprob::wr_from` does. A training game's own result is taken
  out of the tables for its own rows.
- Left out: remakes (`gameEndedInEarlySurrender` or `teamEarlySurrendered` on any player) and
  games under 5 min. Surrenders at 15 and later stay in: they are real outcomes.
- Rows: every frame before `GAME_END` (frame 0 included); events are sorted by timestamp and a
  frame counts every event at or before its timestamp.
- Split: 20 % of games held out by a hash of the match id; five disjoint fifths of the training
  games for the sign check (`features.fold`).
- Fit: hat basis over the knots `0, 10, 15, 20, 25, 30, 40`, L2 on each feature's RMS scale
  (`--l2`, default 1), Newton steps with step halving, each game weighing 1 in total. A feature
  whose |x|-weighted mean coefficient does not have the same sign in all five fifths is dropped
  and the rest refitted on the whole training split; `gold` dropping stops the run.
- Report: coefficients, fold signs, per 5-minute bucket (games, rows, log loss, Brier, ECE,
  accuracy, beside the gold-only model), a 10-bin reliability table, accuracy at 10 / 15 / 20 / 25.
- Gates: held-out ECE ≤ 2 points overall, ≤ 3 in every 5-minute bucket from 5:00 that has at
  least 1,500 held-out games (smaller buckets are reported, not gated), log loss below gold-only.

The gates sit close to sampling noise at the spec's 10k games. On simulated games whose
probabilities are exactly right, the true coefficients score an overall ECE of 2.7 points on
1,000 held-out games, 1.5 on 2,000 and 0.75 on 4,000, and about 3 points per 5-minute bucket
with 1,000 games in it, 1.8 with 2,000 (hence the 1,500-game floor for a bucket's gate). 10k
crawled games give 2,000 held out: a sound model passes, with little margin. Crawl 20k if the
first fit misses a gate by a small amount.

## Tests

```
py -3.12 -I tools/winprob/test_winprob.py
```

Python against the Rust sources (keys, constants, the feature formulas, read from
`winprob.rs`, `consts.rs`, `fights.rs`, `lolalytics.rs`), features on a hand-built timeline,
loud failures on unknown shapes, a fit on 20,000 simulated games from a known table, the TOML
and cases under the Rust schema, and `cargo run -q -p lol-coach --example winprob_replay` on
that TOML (`WINPROB_SKIP_CARGO=1` leaves it out; it uses `target-build` and 2 jobs). The
simulation and cargo are skipped while League runs. Nothing is written into the repo.

## Unverified until a key exists

Each item is an assumption in `riot_format.py` (or in how `features.py` reads it) that plan T0b
was to confirm. "check" says how; most are covered by `features.py --check` on the probe crawl.

1. `league-v4` `/lol/league/v4/entries/RANKED_SOLO_5x5/{tier}/{division}?page=N` and the
   Master / Grandmaster / Challenger `…leagues/by-queue/RANKED_SOLO_5x5` (`entries[]`) carry
   `puuid` on every entry; an empty page ends a division. Check: the probe crawl finds players
   (a `FormatError` "no field 'puuid'" otherwise).
2. `match-v5` `/matches/by-puuid/{puuid}/ids` takes `queue`, `startTime` (epoch s), `start`,
   `count` ≤ 100. Check: the probe crawl stores games of the asked queue only.
3. Match fields: `metadata.matchId`; `info.gameVersion` ("16.22.x.y"), `info.mapId` (11 / 12),
   `info.queueId`, `info.gameDuration` in seconds; `info.participants[]` with `participantId`
   1–10, `teamId` 100 / 200, `championId`, `gameEndedInEarlySurrender`, `teamEarlySurrendered`,
   `item0`…`item6`; `info.teams[]` with `teamId` and `win`. Check: `--check` parses them and
   prints "duration within 60 s of the last frame: N of N".
4. Remakes are flagged by `gameEndedInEarlySurrender` / `teamEarlySurrendered`, and a 15-minute
   surrender is not. Check: `--check` prints the left-out games with their length; remakes are
   about 3–4 min, nothing near 15.
5. Timeline: `info.frameInterval` = 60000; `info.frames[]` with `timestamp` (ms),
   `participantFrames` keyed "1"…"10" with the match's participant ids, each with integer
   `level`, `minionsKilled`, `jungleMinionsKilled`, `totalGold`, `currentGold`; `events[]` per
   frame (which frame an event sits in does not matter). Check: `--check` parses them; "seconds
   between frames" shows 60 and one shorter final gap.
6. The last frame is at the game's end and the timeline ends with `GAME_END` carrying
   `winningTeam`, which agrees with `info.teams[].win`. Check: `--check` raises on a disagreement.
7. Item events: `ITEM_PURCHASED`, `ITEM_SOLD`, `ITEM_DESTROYED` with `participantId`, `itemId`;
   `ITEM_DESTROYED` covers used consumables and the components a purchase consumes, with the
   purchase's exact timestamp; `ITEM_UNDO` with `beforeId` = X, `afterId` = 0 undoes buying X and
   gives the components back without further events; `beforeId` = 0, `afterId` = X undoes
   selling X; transforms (Manamune → Muramana, the support quest) arrive as events that leave
   the final item held; rune items (Biscuits, Stopwatch) need no event. Check: "final items
   rebuilt from the events: N of N players … N same item gold"; the worst mismatches are listed.
8. `CHAMPION_KILL` with `killerId` (0 for an execution) and `victimId`. Check: parsed by
   `--check`; executions show as kills by 0.
9. `BUILDING_KILL` with `buildingType` `TOWER_BUILDING` / `INHIBITOR_BUILDING` and `teamId` = the
   side that **lost** it; nexus turrets are towers (`towerType` `NEXUS_TURRET`). Check: "building
   teamId is not the killer's team" and "nexus turrets fall on the losing side", N of N.
10. `ELITE_MONSTER_KILL` with `monsterType` `DRAGON` / `BARON_NASHOR` / `HORDE` / `RIFTHERALD` /
    `ATAKHAN`, `monsterSubType` `FIRE_ / WATER_ / EARTH_ / AIR_ / HEXTECH_ / CHEMTECH_DRAGON` or
    `ELDER_DRAGON`, and `killerTeamId` = the side credited; one `HORDE` event per grub. Check:
    "monster killerTeamId is the killer's team", N of N; an unknown type or subtype raises.
11. `DRAGON_SOUL_GIVEN` with `teamId`, given to a side with four elemental dragons. Check: a
    `FormatError` otherwise.
12. Ignored event types: `PAUSE_START`, `PAUSE_END`, `SKILL_LEVEL_UP`, `LEVEL_UP`, `WARD_PLACED`,
    `WARD_KILL`, `TURRET_PLATE_DESTROYED`, `CHAMPION_SPECIAL_KILL`, `CHAMPION_TRANSFORM`,
    `OBJECTIVE_BOUNTY_PRESTART`, `OBJECTIVE_BOUNTY_FINISH`, `FEAT_UPDATE`. Any other type stops
    the run with "unknown event type"; add it to `USED_EVENTS` or `IGNORED_EVENTS` after looking
    at it. Check: "event types" from `--check`.
13. Limits: 20 requests / s and 100 / 2 min on a dev key, `Retry-After` (s) on a 429, 401 / 403
    for a bad or expired key. Check: the crawl's request count per hour (~3,000).
14. Data Dragon has a version `<patch>.x` whose `item.json` costs match the servers'. Check:
    the item rebuild in 7 (a cost change shows as a constant offset).

Not about the API, also unchecked: that the Live Client's `creepScore` equals `minionsKilled +
jungleMinionsKilled`, that the Live Client's `respawn_in` matches `fights::respawn` at the death's
level, and that lolalytics buckets games by length the way `features.bucket` does.
