# winprob

Fits the in-game win probability (`crates/coach/src/winprob.rs`, coefficients in
`crates/coach/winprob.toml`) from Riot match-v5 timelines. Spec:
`docs/superpowers/specs/2026-11-11-win-probability-design.md` (section 2 and "Testing"); plan task T3.

**Status: run against the Riot API on 2026-11-12** (patch 16.22, EUW, a dev key). The probe crawl
and `features.py --check` are plan task T0b; what they showed is under "What the real API
showed". The Riot facts are also in the spec's "Facts".

Python is the system 3.12 with `requests numpy`, run as `py -3.12 -I` (`python3 -I` on Linux).
Dev data lives outside the repo in `%LOCALAPPDATA%\lol-coach\dev\winprob\` (on Linux
`$XDG_DATA_HOME` or `~/.local/share`, then `lol-coach/dev/winprob/`): `raw\` (gzipped match and
timeline JSON, about 73 KB a game), `crawl_state.json`, `item-<version>.json` (Data Dragon) and
`rows.npz`.

| file | does |
|---|---|
| `riot_format.py` | every endpoint, field name, event type and enum value the tools assume, in one place |
| `crawl.py` | ladder players (Emerald+, the highest first) → match ids → match + timeline, resumable, rate limited |
| `features.py` | rows (one per minute frame, blue's side) with the Rust features; `--check` runs the T0b checks |
| `fit.py` | the fit, the report, the gates; writes the TOML and the parity cases only when the gates pass |
| `nightly.py` | one scheduled run: crawl the newest patch for a while, refit when enough games are in, drop old raw games |
| `test_winprob.py` | offline tests (below) |

## Run

```
riot-api-key.txt at the repo root  (the key alone, git-ignored; or --key-file, or env RIOT_API_KEY; docs/riot-api-key.md)
tasklist | findstr /i "League"     (nothing long while a game runs)

py -3.12 -I tools/winprob/crawl.py --games 20 --out %LOCALAPPDATA%\lol-coach\dev\winprob\probe
py -3.12 -I tools/winprob/features.py --check --raw %LOCALAPPDATA%\lol-coach\dev\winprob\probe\raw --limit 20
```

The check must end with every agreement line "N of N" and the teams' rebuilt item gold within
its tolerance; it exits 1 otherwise, and any `FormatError` names the match, frame and field. On
a new patch, fix `riot_format.py` (and the spec's "Facts") from what the replies show, then:

```
py -3.12 -I tools/winprob/crawl.py --games 10000            (about 10 h on a dev key; rerun it after a stop or a new key)
py -3.12 -I tools/winprob/features.py --check --limit 500
py -3.12 -I tools/winprob/features.py                       (→ rows.npz)
py -3.12 -I tools/winprob/fit.py                            (→ crates/coach/winprob.toml, fixtures/coach/winprob_cases.json)
cargo test -p coach -q --test winprob                       (parity: the cases reproduce in Rust to 1e-4)
cargo run -p lol-coach --example winprob_replay             (the fit over the recorded live games)
```

`crawl.py` options: `--platform euw1 --region europe` (routing), `--queue 420` (450 for ARAM),
`--patch 16.22` (default: newest on Data Dragon; matches on other patches are not stored),
`--days 14`, `--out DIR`, `--hours H` (stop after that long), `--key-file FILE`, `--no-apex`.
Players are read from the top: Challenger, Grandmaster and Master, then Diamond and Emerald one
page of a division after another. The top three leagues alone fill 20,000 games, so a fit is in
practice on Master and above; `--no-apex` starts at Diamond. Match ids come
newest first, so a match of an older patch ends that player's list. A dev key expires after 24 h: the crawler saves its state and stops on
401 / 403. `features.py --mode aram` and `fit.py` on its rows write the `[aram]` table and keep
an existing `[rift]` one. `fit.py --out X --cases Y` writes elsewhere; rows marked synthetic are
refused for any path inside the repo.

## Every night

`nightly.py` is one run for a scheduler, the same on Linux and Windows (written and tested on
Windows; not yet run on Linux):

1. crawls the newest patch (Data Dragon) for `--hours 2`, up to `--games 20000`, into
   `DIR/<patch>/`; each patch has its own directory and crawl state. A crawl started by hand in
   `DIR` itself goes on there while its patch is the newest;
2. once `--fit-at 10000` games are stored, and again every `--refit-every 2500` more, runs
   `features.py` and `fit.py` into `DIR/<patch>/fit/` (`winprob.toml`, `winprob_cases.json`).
   The gates decide whether anything is written; it never writes into the repo. Not while League
   of Legends runs: the next run fits;
3. with `--publish DIR`, copies the newest patch's fitted table to `DIR/winprob.toml`; when
   `DIR` is a git checkout it also commits (`winprob <patch>: <games> games`) and pushes. A push
   that failed is tried again the next night;
4. removes `raw/` and `rows.npz` of patches older than the newest `--keep 2` (0 keeps all); the
   fitted tables stay.

Everything goes to `DIR/nightly.log`. `DIR/nightly.lock` keeps two runs apart. Exit status 1 when
the crawl, the features, the fit or the push failed. A new patch can bring an event shape the tools do not
know: then the fit fails, the log names the match and the field, and the crawl goes on.

It needs a key that lasts, a personal key (`docs/riot-api-key.md`, section 2). With a dev key the
crawl stops on 401 a day after the key was made, until the file holds a new one. Disk: about
1.5 GB for a patch of 20,000 games, so 3 GB with two patches kept.

Setup: a checkout of the repo, the key in `riot-api-key.txt` at its root, and Python 3.11 or newer
with `requests` and `numpy` where `-I` sees them (system packages or a venv, not `pip --user`):

```
sudo apt install python3-requests python3-numpy                (Debian, Ubuntu)
chmod 600 riot-api-key.txt
python3 -I tools/winprob/nightly.py --hours 0.05               (a three-minute try)
```

Linux, cron (`crontab -e`), every night at 03:30:

```
30 3 * * * cd $HOME/lol-coach && /usr/bin/python3 -I tools/winprob/nightly.py
```

or a systemd user timer (`systemctl --user enable --now winprob.timer`, and
`loginctl enable-linger $USER` so it runs with nobody logged in):

```
# ~/.config/systemd/user/winprob.service
[Service]
Type=oneshot
WorkingDirectory=%h/lol-coach
ExecStart=/usr/bin/python3 -I tools/winprob/nightly.py

# ~/.config/systemd/user/winprob.timer
[Timer]
OnCalendar=*-*-* 03:30
Persistent=true

[Install]
WantedBy=timers.target
```

Windows, Task Scheduler:

```
schtasks /Create /TN "lol-coach winprob" /SC DAILY /ST 03:30 /TR "py -3.12 -I C:\path\to\lol-coach\tools\winprob\nightly.py"
```

### The app downloads the table

At its start and every 6 hours the app (`src-tauri/src/winprob.rs`) reads `winprob.toml` from the
repo's `winprob-data` branch on GitHub, or from the address in env `LOL_COACH_WINPROB_URL`. The
table is used when `Model::fault` finds nothing wrong (a fit, rising knots, features this build
knows, one number per knot) and it is newer than the built-in one: a later patch, or the same
patch and more games. The last good download stays in the app's disk cache and is used when
there is no network. Anything else leaves the built-in table in use.

The branch exists (made empty on 2026-11-12, no history shared with `master`). The machine that
fits checks it out once:

```
git clone --branch winprob-data --single-branch git@github.com:PaulDotSH/lol-tft-coach.git ~/winprob-data
```

and its scheduler line gets `--publish $HOME/winprob-data`. A deploy key with write access is
enough for the push (GitHub, the repo's Settings, Deploy keys). Until the branch has a table the
app logs "win probability table not downloaded" and goes on with the built-in one.

The built-in table is still set by hand, for a build that works offline from its first start:
copy `DIR/<patch>/fit/winprob.toml` to `crates/coach/winprob.toml` and `winprob_cases.json` to
`fixtures/coach/`, run `cargo test -p coach -q --test winprob`, rebuild.

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

## What the real API showed (2026-11-12, 16.22)

The documented format held for everything but the items and the soul event. `riot_format.py`
and `features.py` follow the replies now; the differences:

1. `ITEM_PURCHASED` comes with `participantId` 0 twice a game: each support's World Atlas at
   0:00. `features.adopt` gives them to the players who later lose that item, or end with it,
   without having bought it.
2. An item can change into another with no purchase event, only an `ITEM_DESTROYED` of the old
   one: the support quest's stages and its upgrade, upgraded boots. `Values.after` (from Data
   Dragon's `specialRecipe`, and `from` with a base cost of 0) names what a destroyed item becomes
   when its owner bought nothing at that moment. Of the support item's five upgrades the events
   do not say which; they cost the same.
3. Items are destroyed that the events never gave: components of an item bought whole, rune and
   quest items. `ITEM_UNDO` sometimes has neither `beforeId` nor `afterId`. These are counted
   (`Inventories.odd`), not errors.
4. Match participants have `roleBoundItem` beside `item0`…`item6`: a role quest reward, or the
   boots of a player with a boots slot. `final_items` counts it.
5. `DRAGON_SOUL_GIVEN` is sent with `teamId` 0 when the soul's kind is announced (after the
   second dragon) and with a side's id when it takes its fourth. Only the second is the soul.
6. A winner can lose a nexus turret, so that line of `--check` asks for most, not all.
7. `ELITE_MONSTER_KILL` has `killerTeamId` 300 for a monster no side killed: a Rift Herald (88
   of 1,122) and a Void Grub (8 of 3,609). Those grubs are nobody's.
8. Viego: each possession sends `ITEM_DESTROYED` for all he holds, then for all the body held,
   and nothing when his own items are back. For him (`POSSESSOR`) a destroyed item counts only
   when he bought something at that timestamp (its components).
9. A Tear item bought with the Tear already full is purchased and destroyed at one timestamp
   and is its upgrade from then on (`Values.at_once`).

So the item rebuild is close, not exact, and `--check` passes on a tolerance: a team's rebuilt
item gold within 2 % of its final items' on average and 10 % at worst. On 500 games (489
kept): 4,777 of 4,890 players exact in gold, teams off 0.11 % on average and 4.2 % at worst. Most
of the rest are mid laners with the quest reward (`roleBoundItem` 1206), whose boots change with
no event that says to what. Gold spent (`totalGold − currentGold`) would be 7.6 % high (first
20 games).

Confirmed as assumed: the league and match-id endpoints, the match and frame fields,
`frameInterval` 60000 with a last frame at the game's end, `GAME_END.winningTeam`,
`BUILDING_KILL.teamId` as the side that lost it, `ELITE_MONSTER_KILL.killerTeamId`, one `HORDE`
event per grub, `killerId` 0 for executions and minions, the rate limits. The crawl stores about
1,000 games an hour: half the match ids are from another patch and cost a request each.

Seen later, in the first 1,221 games of the crawl: an Elder dragon is `DRAGON` with
`monsterSubType` `ELDER_DRAGON` (59), a remake has `gameEndedInEarlySurrender` and lasts 66 to
134 s (9 of 500), `CHAMPION_TRANSFORM` exists (ignored), and there is no Atakhan event at all.

Still unverified: `PAUSE_START` and `FEAT_UPDATE` (ignored types, not seen), ARAM (queue 450). Not about the API: whether the Live Client's `items` include the
role-bound slot, that its `creepScore` equals `minionsKilled + jungleMinionsKilled`, that its
`respawn_in` matches `fights::respawn`, and that lolalytics buckets games by length the way
`features.bucket` does.
