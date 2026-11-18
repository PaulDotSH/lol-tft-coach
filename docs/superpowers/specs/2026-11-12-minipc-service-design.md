# Minipc service: player data, player lookups, match crawl

Status: design, 2026-11-12. Part A (HTTP service) in progress; parts B to E not started.

## Goal

One small Rust service on the always-on Linux minipc that

1. keeps each player's app data (history, recaps, ranked snapshots, settings, calibration, TFT
   data), stored per player username (Riot ID), so it is on the minipc and in its backup;
2. looks up the players of the user's games with the Riot API and keeps the result per Riot ID;
3. crawls ranked games for the win-probability fit without a break, on several servers at once,
   and serves the fitted table to the app.

Logs, debug dumps, caches, models and the Coachless tokens stay on the PC.

Stopping condition: the service runs under systemd on the minipc behind HTTPS, the app on the PC
reads and writes its player data there and falls back to its local copy when the minipc cannot be
reached, the minipc backup script includes the player data, and the crawl holds at most 100,000
games on disk.

## Decisions (user, 2026-11-12)

- Rust, low resource use, runs all the time on the minipc. The crawler moves into the service
  (`tools/winprob/crawl.py` and `nightly.py` stay for a PC).
- Data on the minipc: history, recaps, ranked, settings, TFT data, and lookups of the players in
  my games. Per player username.
- The app reaches the service over the public internet: HTTPS, a secret token.
- Match data: the two newest patches, 100,000 games in all at most (about 7.3 GB at 73 KB a game;
  50,000 at first, raised by the user on 2026-11-12: the minipc has 498 GB free).
  On a new patch the new patch's games come in and the old patch's go out one for one.
- The minipc backup script also backs up the player data (not the match data, which can be
  crawled again).
- Implementation by subagents; setup over SSH from the PC.
- Servers: EUW and Korea, not NA ("bad players compared to Korea and EUW"; 2026-11-12, after the
  first minutes of the service had crawled NA). `deploy/minipc/set-servers.sh` sets the list and
  removes a dropped server's games and state, so no fit reads them.

## Facts (measured 2026-11-12 on the user's key)

- App limit 20 requests / 1 s and 100 / 120 s, sent in `X-App-Rate-Limit`; the count so far in
  `X-App-Rate-Limit-Count`. A 429 carries `Retry-After` (seconds) and `X-Rate-Limit-Type`.
- The limit is counted for each host. `euw1`, `eun1`, `na1`, `kr` (platform hosts: league-v4,
  champion-mastery-v4, summoner-v4) each have their own count, and so do `europe`, `americas`,
  `asia` (regional hosts: match-v5, account-v1). EUW and EUNE matches both go through `europe`,
  so a second European server adds no match throughput.
- Each method also has its own limit (`X-Method-Rate-Limit`; the app's page on the developer
  portal lists them): match-v5 2,000 / 10 s, league entries pages 50 / 10 s, the three top
  leagues 30 / 10 s and 500 / 10 min, account-v1 1,000 / min, summoner-v4 1,600 / min, the
  by-puuid league and mastery methods 20,000 / 10 s. All are far above the app limit, which is
  the one that binds; a 429 of either kind is handled by its `Retry-After`.
- Player ids (puuids) are encrypted for each key; match ids are the same for every key.
- The Python crawler takes about 51 MB and under 1 % of a core for one server; 2.3 requests for
  each stored game, about 1,300 games an hour for one region.
- Replies saved (one player, made anonymous) in `fixtures/riot/`: `account_by_riot_id.json`,
  `account_not_found.json` (404), `league_entries_by_puuid.json`, `mastery_top.json`,
  `summoner_by_puuid.json`, `rate_limited.json` (429).
- Endpoints: `/riot/account/v1/accounts/by-riot-id/{gameName}/{tagLine}` (regional),
  `/lol/league/v4/entries/by-puuid/{puuid}`, `/lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/top?count=5`,
  `/lol/summoner/v4/summoners/by-puuid/{puuid}` (platform). The crawl's endpoints and reply fields
  are in `tools/winprob/riot_format.py`.

## Shape

New crate `crates/server`, package and binary `lol-coach-server`. It depends on no other crate
of the workspace (it must build on Linux without Tauri or the Windows crates). axum 0.8 on
tokio, tower-http, reqwest (rustls), serde, toml, thiserror, tracing. Plain data and free
functions; `lib.rs` has `pub fn app(state) -> Router` so tests drive the router without a port.

```
crates/server/src/
  main.rs      config, tracing, state, background tasks, serve, graceful shutdown
  lib.rs       AppState, app()
  config.rs    Config from a TOML file
  error.rs     AppError, AppResult
  auth.rs      bearer token
  players.rs   player key from a Riot ID, the file store
  riot.rs      Riot client: hosts, limiter, replies
  lookup.rs    player lookup and its cache
  winprob.rs   the fitted table to serve
  crawl.rs     part B
  routes/      one module for each group of routes
```

### Config (`server.toml`, path from `--config` or `LOL_COACH_SERVER_CONFIG`)

```toml
listen = "127.0.0.1:8787"
data_dir = "/var/lib/lol-coach"
tokens = ["..."]                      # at least one, each 32 characters or more
riot_key_file = "/etc/lol-coach/riot-api-key.txt"   # missing: lookups and crawl are off
lookup_ttl_hours = 12
max_file_mb = 32

[crawl]                               # part B; left out: no crawl
servers = ["euw1/europe", "kr/asia"]
max_games = 100000
fit_at = 10000
refit_every = 2500
python = "python3"
tools_dir = "/opt/lol-coach/tools/winprob"
```

Checked at start; a bad value stops the service with a message that names the key. The Riot key
and the tokens are never logged.

### Data on disk

```
<data_dir>/players/<player>/<kind>/<name>        the app's files
<data_dir>/players/<player>/lookup/<platform>.json
<data_dir>/winprob/<patch>/raw/*.json.gz          part B, as crawl.py writes them
<data_dir>/winprob/<patch>/<platform>.json        part B, crawl state
<data_dir>/winprob/<patch>/rows.npz, fit/winprob.toml, fit/winprob_cases.json
```

`<player>` is the Riot ID `name#tag`, trimmed, lowercased, then percent-encoded so that only
`a-z 0-9 - _ .` stay as they are (so `Some Player#EUW` is `some%20player%23euw`). A Riot ID
without `#`, with an empty part, or longer than 64 characters is refused. `<kind>` and `<name>`
match `[A-Za-z0-9][A-Za-z0-9._-]{0,127}`. Nothing else reaches the file system, so no path can
leave `players/`. The backup takes `<data_dir>/players`.

### HTTP API

All routes but `/v1/health` and `/v1/winprob.toml` (coefficients, nothing private) need
`Authorization: Bearer <token>`, compared in constant time; 401 otherwise. Errors are JSON `{"error": "..."}`. `{player}` in a path is the percent-encoded Riot ID.

| Route | Does |
| --- | --- |
| `GET /v1/health` | 200 `{"ok": true}` |
| `GET /v1/players/{player}/files` | every file of the player: `[{kind, name, size, modified, sha256}]`, `modified` in unix seconds; `?kind=` for one kind; an unknown player is `[]` |
| `GET /v1/players/{player}/files/{kind}/{name}` | the bytes, `ETag` the sha256; 404 |
| `PUT /v1/players/{player}/files/{kind}/{name}` | stores the body (written to a temporary file, then renamed); 413 above `max_file_mb`; replies `{size, modified, sha256}` |
| `DELETE /v1/players/{player}/files/{kind}/{name}` | 204; 404 |
| `GET /v1/lookup/{platform}/{player}` | the lookup below; `?fresh=1` skips the cache; 404 when Riot has no such player; 503 when there is no Riot key; 400 for an unknown platform |
| `GET /v1/winprob.toml` | the fitted table of the newest patch that has one (`winprob/<patch>/fit/winprob.toml`, patches compared as numbers); 404 when none |
| `GET /v1/status` | `{version, players, riot_key, crawl}`; `crawl` is null until part B |

The server does not read the app's files: kinds are the app's business (part D). Last write wins.

### Lookup

```json
{"riot_id": "Some Player#EUW", "platform": "euw1", "fetched_at": 1794456000, "level": 612,
 "solo": {"tier": "CHALLENGER", "division": "I", "lp": 3112, "wins": 506, "losses": 408, "hot_streak": false},
 "flex": null,
 "mastery": [{"champion_id": 12, "level": 40, "points": 512345, "last_played": 1794400000}]}
```

Four requests for a player not seen before (account, league entries, mastery, summoner; the last
three at once), three after that (the puuid is kept in the lookup file with the key it was read
with: on "Exception decrypting" it is read again). Kept `lookup_ttl_hours`; an older file is
served when Riot cannot be reached, with `"stale": true`. `solo` is `RANKED_SOLO_5x5`, `flex` is
`RANKED_FLEX_SR`; unranked is null. Platform to region: br1, la1, la2, na1 → americas; euw1, eun1,
tr1, ru, me1 → europe; kr, jp1 → asia; oc1, sg2, tw2, vn2 → sea.

### Riot client and limiter

One limiter for each host, shared by lookups and crawl: sliding windows from `X-App-Rate-Limit`
(20 / 1 s and 100 / 120 s until a reply says otherwise), a 429 holds the host for its
`Retry-After`, a 5xx is tried again with a growing wait, 401 / 403 is "key rejected" (lookups
answer 503, the crawl stops and tries again in 10 minutes). Two priorities: a lookup takes the
next free slot; the crawl waits while a lookup waits and spaces its own requests evenly (one
every window / limit seconds of the longest window), so a lookup never waits behind a burst.
Timeouts on every request. The base URLs come from config-free constants that tests can replace
by a local fake server.

## Accounts: one player's data is that player's (user, 2026-11-12)

The user's rule: every account has its own data, and the service believes an app is a player's
when the app sends a match that player has just finished, the match checks out against Riot,
and nobody sent that match for that player before.

- The app makes a random account key (32 bytes, hex) for each account on each PC and keeps it
  beside that account's files. It is a secret like the token.
- `POST /v1/players/{player}/claim` (bearer token) with `{platform, game_id, champion_id, win,
  duration_s, account_key}`. The service reads the player's puuid (account-v1) and the match
  (`<PLATFORM>_<game_id>`, match-v5) and accepts when the player is one of the match's
  participants, that participant's champion and result are the ones sent, the duration is within
  10 s, the match ended at most `claim_max_age_hours` (24) ago, and this match was not used for
  this player before. Then the sha256 of the key is added to `players/<player>/account.json`
  (several keys can be there: one for each PC) and the match is remembered as used.
  Replies: 200 `{"claimed": true}`; 202 `{"claimed": false, "retry": true}` while Riot does not
  have the match yet (the app asks again, for up to 10 minutes); 403 when it does not check out;
  409 when the match was used already. A claim with a key that is already there is 200 and uses
  no match.
- The player's files (`/v1/players/{player}/files…`) need, besides the bearer token, the header
  `X-Account-Key` with a key of that player; 403 otherwise, also for a player nobody claimed.
  Lookups and the table do not need it.
- What it is worth: the bearer token decides who can reach the service at all (the user and
  the friends they give it to); the account key keeps those people's data apart and stops one
  from writing into another's. It is not proof against a holder of the token who wants in:
  they can claim an account with a match its owner's app never sent (played on another PC, or
  before the owner installed the app). The real proof is Riot's sign-in for third parties, which
  Riot gives to approved products only. A wrong claim is undone by deleting
  `players/<player>/account.json` on the minipc.
- A player who changes their Riot ID gets a new, empty directory; the old one is moved by hand.

## Part B: crawl in the service

A task for each server in `[crawl] servers` (one server for each region; two of one region is a
config error). The behaviour of `tools/winprob/crawl.py`, which stays the reference and whose
tests name the cases: ladder from the top (Challenger, Grandmaster, Master, then Diamond and
Emerald pages in turn), each player's match ids newest first, a match and its timeline when the
match is on the newest patch (Data Dragon `versions.json`, asked again every hour), a match of an
older patch ends that player's list, remembered match ids are not fetched twice, state saved
after every game (temporary file, then rename), "Exception decrypting" drops the players once
and reads the ladder again. Files are `<matchId>.match.json.gz` and `<matchId>.timeline.json.gz`
in `winprob/<patch>/raw/`, the JSON as Riot sent it, so `features.py` reads them.

Storage: the newest patch is crawled to `max_games`, each server to its share. Patches older
than the newest two lose their `raw/` and `rows.npz`. After a game of the newest patch is stored,
while the two patches hold more than `max_games` games, the oldest stored game of the older patch
is deleted.

Fit: with `fit_at` games of the newest patch and again every `refit_every` more, the service runs
`<its own executable> features --raw … --out rows.npz --patch … --items … --bands <patch dir>` and
`<python> -I <tools_dir>/fit.py --rows … --out … --cases …` as child processes (the fit stays in
Python: numpy, tested, and it runs minutes a day). The `features` subcommand
(`crates/server/src/features.rs`) is `features.py` in Rust, checked against it to the bit on the
15,695 games of 16.22: it reads each game's match file once to pick the games and build the
outlook tables, then each kept game's timeline once, writing rows to scratch files (four threads at
most) and then `rows.npz` (a zip of deflated `.npy` members, written by hand) that `np.load`
reads as before, with one member more, `band`. It holds
about 0.4 KB a game (27 MB at 15,695 games against about 1 GB for `features.py`) and took 11 s
there against 150 s. `--bands` reads `"bands": {match id: "DIAMOND" | "EMERALD"}` from the state
files; other games are `APEX`, and `fit.py` reports the held-out scores for each band. `fit.py`
writes the table only when its gates pass; `/v1/winprob.toml` then serves it. `/v1/status` shows
games for each server and patch, the last fit and whether it passed.

## Part C: on the minipc

Over SSH: Rust toolchain if missing, `RUSTFLAGS="-C target-cpu=native" cargo build --profile production -p lol-coach-server`
on the minipc itself, binary to `/usr/local/bin`, a `lol-coach` user, `/etc/lol-coach/server.toml`
and the key file (mode 600), `/var/lib/lol-coach`, a systemd unit (`Restart=always`, hardened:
`NoNewPrivileges`, `ProtectSystem=strict`, `ReadWritePaths=/var/lib/lol-coach`), python3 with numpy
and requests for the fit, the repo's `tools/winprob` in `/opt/lol-coach`. HTTPS by the reverse
proxy the minipc already has, or Caddy if it has none; the service itself listens on localhost.
The backup script gets `/var/lib/lol-coach/players`.

Found on the minipc (2026-11-12, over SSH as `minipc@192.168.0.226`, key auth): Arch Linux, Rust
1.99.1, Python 3.14 with numpy and without requests (so the service fetches Data Dragon's
`item.json` itself and passes `features.py --items`), 500 GB free. sudo needs a password, and
the user runs root steps as one script themselves (`ssh -t … sudo bash script`). The house
layout, which this follows instead of the paths above where they differ: source and build in
`~/lol-coach/src`, binary in `/opt/lol-coach/bin`, a system unit `lol-coach-server.service` as
`User=caddy` on `127.0.0.1:<port>` hardened like `septica.service`, a site block for a subdomain
of foarte.top in `/etc/caddy/Caddyfile` (Caddy is there; DNS by `/usr/local/bin/cf-ddns.py`;
no Authelia in front of this one: the app is not a browser, the token is the lock), and a group
in `~/.local/bin/stack-backup.sh`, which copies into `~/backup/stack-migration`. Cloudflare is
in front of foarte.top: whether it lets a non-browser client through has to be tried.
The address is `lol.foarte.top` (user, 2026-11-12), port 8787. The files are in
`deploy/minipc/` (`install.sh` for the root steps, the unit, the Caddy block, a README); each
edit of the script was tried on copies on the minipc without root.

## Part D: the app keeps its player data on the minipc

What the app has today (read 2026-11-12), all under `%LOCALAPPDATA%\lol-coach` unless said:
`history\<gameId>.v<N>.json` (256 files, 1 MB), `recaps\<unix>.json` (78 files), `ranked\snapshots.json`
(rewritten whole), `tft\looks.bin`, `calib.json`, and `%APPDATA%\lol-coach\settings.toml`. None
of it is kept apart by account.

- **Settings.** `server_url` and `server_token` in `model::Settings`, both empty by default
  (empty URL: no sync, nothing changes). Two fields in the settings panel, the token a password
  field. The token is a secret like the Coachless tokens: never in a log, a debug dump or an
  error text.
- **The local files stay the working copy.** Nothing that reads or writes them changes; the app
  works with the minipc off. A new module `src-tauri/src/remote.rs` copies both ways.
- **The player** is `gameName#tagLine` of the League client's `/lol-summoner/v1/current-summoner`,
  as a key the way the service makes it (`players.rs`).
- **Each account has its own files on the PC too** (user, 2026-11-12): `history`, `recaps` and
  `ranked` move from `%LOCALAPPDATA%\lol-coach\<kind>` to `%LOCALAPPDATA%\lol-coach\players\<key>\<kind>`.
  The files that are there today go, once, to the first account the client reports. Until the
  client has named a player in this run, the account used last (kept in `players\last.txt`) is
  the one read and written, so a recap is never lost. `tft\looks.bin`, `calib.json` and the
  settings belong to the PC, not to an account, and stay where they are.
- **Claim.** The account's key is `players\<key>\account.key`, made on first use. When a game
  ends, and at the first sync if the newest history game ended less than 24 hours ago, the app
  sends the claim for that game (see "Accounts") with the game id, champion, result and
  duration it has from the client; on 202 it asks again every 30 s for 10 minutes. Until the
  service has accepted the key, nothing else is synced for that account.
- **Kinds.** `history`, `recaps`, `ranked`: every regular file of that account's local directory
  whose name the server accepts (`[A-Za-z0-9][A-Za-z0-9._-]{0,127}`) and that is at most 32 MB;
  no subdirectories. `settings`: `settings.toml` with `server_url` and `server_token` left out
  (a copy for a new PC; `calib.json` and `tft\looks.bin` are this PC's and are not sent).
- **A sync.** `GET /v1/players/{player}/files`, sha256 of the local files (kept in memory by
  size and modified time). Same hash: nothing. Only here: upload. Only there: download. Both,
  different: the newer wins, local modified time against the server's `modified` (which is when
  that version was uploaded). `settings` is upload only, except on a PC that has no
  `settings.toml` yet, which downloads it: a running app's settings are not replaced under it.
  Nothing is ever deleted on either side. A download is written to a temporary file and renamed.
  `ranked\snapshots.json` is the one file every PC keeps writing, so there the two versions are
  joined (every snapshot of either, once, by time and queue) and the result goes to both sides.
  After a round that brought files in, the app reads the account's files again.
- **When.** Once the client is connected and the player known, then every 10 minutes, never
  while a game is running. A failure is logged at info and tried again at the next round.
- **Shown.** The settings panel has one line: when the last sync ran and what it did, or why it
  did not.
- **The table.** `GET /v1/winprob.toml` needs no token (the table is not private), and the app
  reads it from `<server_url>/v1/winprob.toml` when a server is set, else from the
  `winprob-data` branch as today.
- **Tests.** The plan of a sync (what goes up, what comes down, what is left) as a pure function
  with unit tests; one end-to-end test against the real router of `lol-coach-server` (a
  dev-dependency) on a local port with temporary directories: first sync uploads, a second PC's
  directory downloads, the newer of two wins, another player is skipped, the token is not in any
  log line or error.

## Part E: the players of my game in the app

After the game has loaded, the app asks `/v1/lookup` for the nine other players (Riot IDs from
the Live Client API) and shows rank and the mastery of the champion played on the threat board
and the scoreboard. Not in champion select of ranked solo queue: the client hides the names
there and Riot's rules forbid working around that. Design of the display: later, with the user.

## Testing

Tests first. The router through `app()` and `tower::ServiceExt::oneshot` with a temporary data
directory: auth (missing, wrong, right token), the player key (case, spaces, `#`, unicode,
refused forms), the store (put, list, get, delete, 413, names that try to leave the directory),
the lookup against a local fake Riot server that replies with the `fixtures/riot/` files (fresh,
cached, stale on failure, not found, no key, another key's puuid), the table (none, newest patch
wins). The limiter with a fake clock: windows, `Retry-After`, a lookup ahead of the crawl, the
crawl's even spacing, limits from the header. Part B: the cases of `Crawl` and `Nightly` in
`tools/winprob/test_winprob.py`, and the `max_games` rule.

## Not in this design

Accounts and passwords (tokens only), a database (files are enough and easy to back up), sharing
data between players, the fit in Rust.
