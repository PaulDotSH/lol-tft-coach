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
reached, the minipc backup script includes the player data, and the crawl holds at most 50,000
games on disk.

## Decisions (user, 2026-11-12)

- Rust, low resource use, runs all the time on the minipc. The crawler moves into the service
  (`tools/winprob/crawl.py` and `nightly.py` stay for a PC).
- Data on the minipc: history, recaps, ranked, settings, TFT data, and lookups of the players in
  my games. Per player username.
- The app reaches the service over the public internet: HTTPS, a secret token.
- Match data: the two newest patches, 50,000 games in all at most (about 3.7 GB at 73 KB a game).
  On a new patch the new patch's games come in and the old patch's go out one for one.
- The minipc backup script also backs up the player data (not the match data, which can be
  crawled again).
- Implementation by subagents; setup over SSH from the PC.

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
servers = ["euw1/europe", "na1/americas", "kr/asia"]
max_games = 50000
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

All routes but `/v1/health` need `Authorization: Bearer <token>`, compared in constant time; 401
otherwise. Errors are JSON `{"error": "..."}`. `{player}` in a path is the percent-encoded Riot ID.

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
`<python> -I <tools_dir>/features.py --raw … --out … --patch …` and `fit.py --rows … --out … --cases …`
as child processes (the fit stays in Python: numpy, tested, and it runs minutes a day). `fit.py`
writes the table only when its gates pass; `/v1/winprob.toml` then serves it. `/v1/status` shows
games for each server and patch, the last fit and whether it passed.

## Part C: on the minipc

Over SSH: Rust toolchain if missing, `RUSTFLAGS="-C target-cpu=native" cargo build --profile production -p lol-coach-server`
on the minipc itself, binary to `/usr/local/bin`, a `lol-coach` user, `/etc/lol-coach/server.toml`
and the key file (mode 600), `/var/lib/lol-coach`, a systemd unit (`Restart=always`, hardened:
`NoNewPrivileges`, `ProtectSystem=strict`, `ReadWritePaths=/var/lib/lol-coach`), python3 with numpy
and requests for the fit, the repo's `tools/winprob` in `/opt/lol-coach`. HTTPS by the reverse
proxy the minipc already has, or Caddy if it has none; the service itself listens on localhost.
The backup script gets `/var/lib/lol-coach/players`. Open: host, user, domain, the backup script.

## Part D: the app keeps its player data on the minipc

Settings `server_url` and `server_token` (the token stays in `%APPDATA%\lol-coach\settings.toml`,
never in the repo or a log). The local files stay as they are and stay the working copy, so the
app works with the minipc off. The player is the Riot ID the League client reports. Kinds:
`history` (`local_dir("history")`), `recaps`, `ranked` (`snapshots.json`), `tft` (`looks.bin`),
`settings` (`settings.toml` without the token, `calib.json`). At start and after a file is
written: list the player's files, upload what is newer or missing there, download what is newer
or missing here (by sha256 and `modified`). Two accounts on one PC: to be designed with the code
read (today the local files are not kept apart by account). The win-probability table comes from
`<server_url>/v1/winprob.toml` when a server is set, else from the `winprob-data` branch.

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
`tools/winprob/test_winprob.py`, and the 50,000 rule.

## Not in this design

Accounts and passwords (tokens only), a database (files are enough and easy to back up), sharing
data between players, the fit in Rust.
