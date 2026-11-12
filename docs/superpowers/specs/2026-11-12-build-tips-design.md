# Build tips from the crawled games

Status: built 2026-11-12, all four parts, tested offline. Not yet live on the minipc and not
yet seen in the real app; the check against a real client timeline (Part 3) is still open. Builds on the minipc service
(`2026-11-12-minipc-service-design.md`) and the win-probability tools
(`2026-11-11-win-probability-design.md`).

## Goal

Tell the player where their item build differs from what Emerald+ players do on the same
champion and role, and how much slower it is: in the review after a game, and as one silent line
in game. The numbers come from the ranked games the minipc already crawls.

Stopping condition: after a Summoner's Rift game on a champion and role with enough crawled
games, the review shows a Build section with my item times and order beside Emerald+ and
Master+, and at most two tips; in game the item card shows the pace for my next item. With no
table (service not set, pair too rare) both are absent and nothing else changes.

## Decisions (user, 2026-11-12)

- Where: the review first, then a light line in game.
- What is compared: timing, item order, first back, and how the order changes by situation.
- Compared with: Emerald+ (all crawled games), with Master+ beside it.
- Crawl mix: each server takes half its games from the top leagues and half from Diamond and
  Emerald. Every stored game keeps the band it was found in.
- Servers stay EUW and Korea.
- Coachless stays the source of what to build: it has far more games. Buy next, the build path
  and the situation terms from Coachless are not changed and nothing here overrides them. The
  crawl's tables add what Coachless does not give: item times, the first back, and how common
  my path is among Emerald+.

## Facts (measured 2026-11-12)

- One fifth of the 15,695 games crawled on patch 16.22 was read: 581 champion and role pairs;
  about 198 have 200 games or more in the whole crawl and hold 89 % of all player-games, about
  250 have 100 or more. For the most played pairs the middle half of first-item times is under
  three minutes wide, and one first item usually has well over half the games.
- Those 15,695 games were all found through Master+ players: the PC crawl had read 11,000
  players from the three top leagues and was at player 2,650. The service does the same today:
  `crawl.rs` reads Challenger, Grandmaster and Master first and only then Diamond and Emerald
  pages, and the top leagues alone fill a server's share. Without a change no Diamond or Emerald
  game is ever stored.
- The service reads about 1,300 games an hour for one region. Lower-band players are without
  end; top-band games are limited by how much about 11,000 players play.
- The app's review (`MatchDetail`, `crates/clients/src/history.rs`) has per-minute frames with
  `total_gold` and `current_gold` for all ten players, final items, kills and objectives. It has
  no item purchases. The client's timeline does carry `ITEM_PURCHASED` events with
  `participantId`, `itemId` and `timestamp` (`fixtures/clients/match_timeline.json` has one).
  Whether it also carries `ITEM_UNDO` is not known: no real client timeline with an undo is
  saved yet.
- The app already places a live game in Coachless' situations (`crates/coach/src/situations.rs`)
  from its own champion data and the win model. The build tables cannot use that: the builder
  runs in Python on the minipc and has only the games.
- The service starts the fit tools as child processes on one trigger (`fit_at`, then every
  `refit_every` games) and serves `/v1/winprob.toml` without a token. The app downloads that
  table at start and every 6 hours (`src-tauri/src/winprob.rs`).

## Shape

```
minipc                                              PC
crawl (two bands) -> raw games -> lol-coach-server builds -> builds.json  --GET /v1/builds.json-->  app
                                                             coach::builds (pure) -> review, item card
```

Rejected: asking the service about each game (it would have to read the app's formats and be
reachable at review time); computing in the app (the raw games are not on the PC).

The tips say what players usually do and how fast. They never say an item wins more: with a few
hundred games on a path, win-rate differences are mostly noise and follow who was already ahead.
The table carries no win rates.

## Part 1: crawl mix and bands

`crates/server/src/crawl.rs`.

- Two player queues for each server and patch. Top: the three top leagues, as now. Lower:
  Diamond and Emerald pages in the existing order (`ladder_order`).
- `Ladder` keeps its fields for the top queue (`apex_done`, `puuids`, `next_puuid`) and gains
  `lower_puuids` (each player with the tier of the page they came from) and `next_lower`;
  `pages`, `exhausted` and `turn` belong to the lower queue.
  A state file written before this change reads as a top queue in progress and an empty lower
  queue, which is what it is.
- `Saved` gains `bands`: match id to `"DIAMOND"` or `"EMERALD"`, for stored games found through
  a lower-queue player (the tier of the page the player came from). A stored id that is not in
  `bands` is a top-band game, so every game stored so far is top band. Eviction removes the id
  from `bands` too.
- A queue takes players while its band holds under half the server's share and the server is
  under its share. While both may, they take turns, one player each. A server whose top band
  already holds more than half when this goes live gives the lower queue only what is left;
  the mix is half and half from the next patch on.
- When the top queue reaches its end with room left, it reads the top leagues again and starts
  over; games already seen cost no match request. A pass that stored no game makes the top queue
  wait one hour.
- `/v1/status` shows top and lower counts for each server.
- The band is that of the player a game was found through, not of all ten players. No extra
  requests.

A game that ended a player's list (older patch) is kept in `older`, so a later pass stops at it
with no match request.

The per-band fit report (held-out scores for each band, to see whether the table holds below
Master) is not part of this work: it goes in with the port of `features.py` and `fit.py` to Rust,
decided 2026-11-12.

Tests: queue choice and caps (pure function), the old state file read as described, eviction
keeping `bands` in step, the top queue starting over, against the saved Riot replies.

## Part 2: build tables

`crates/server/src/builds.rs` (changed from a Python tool on 2026-11-12, for memory and speed), a
subcommand of the service binary. It reads one game at a time (gzip and JSON into structs with
only the fields it needs) and keeps counters: a first pass over the match files for the damage
shares, a second over matches and timelines.

```
lol-coach-server builds --raw <patch dir>/raw --bands <patch dir> --items items-<version>.json --patch 16.22 --out <patch dir>/builds.json
```

Definitions, all from the match, the timeline and Data Dragon's item file:

- Player: `championId` and `teamPosition` of each of the ten participants. Games under 15
  minutes and remakes are left out.
- Finished item: purchasable on Summoner's Rift, built from components, costs 2,000 gold or
  more, not boots, and no purchasable item builds from it (so Manamune counts, a component does
  not). Finished boots: boots built from Boots. A purchase undone by `ITEM_UNDO` does not count.
  The time of an item is its first purchase.
- First back: the first purchase at 2:00 or later. Its items: every purchase within 30 seconds
  of it. Gold in pocket: `currentGold` in the last per-minute frame before it.
- Enemy damage: each champion's magic share is its magic damage to champions over its physical
  plus magic damage, over all its games in the crawl. A team's value is the mean over its five
  champions. The cuts are the thirds of that value over all games: physical, mixed, magic.
- Lead: my `totalGold` against the enemy in the same position, in the last frame before the
  purchase; behind at 10 % less or worse, ahead at 10 % more or better, else even.

For each champion and role with at least 100 games, for `all` (Emerald+) and, when it also has
100 games, `top` (Master+):

- `slots`: 25th, 50th and 75th percentile of the time of the first, second and third finished
  item, and of finished boots, over the games that have one.
- `first`: first items taken in 5 % of games or more (at most four), with share and the item's
  own time percentiles.
- `second`, `third`: for each listed start, the next items taken in 10 % of its games or more
  (at most three), with share.
- `back`: percentiles of the first back's time and gold in pocket, and the five most bought
  items with the share of first backs that include them.
- `situations`: for the second and the third item, after a listed start, in one damage or lead
  situation with at least 150 games: the most taken item there, when it is not the most taken
  item overall after that start. Most pairs will have none.

The file also carries `patch`, `games`, the game count of each band, the ids of finished items
and boots, each champion's magic share, the two damage cuts and the lead cut. The app reads
these instead of deriving them, so both sides sort a game the same way. Times are seconds. Expected size:
under 1 MB.

Service: the builder runs on the fit's trigger, after the fit, whatever the fit's result, as a
child process of the service's own executable through the fit's runner (its memory goes back when
it exits, and a crash cannot take the service down); its own failure is logged and changes
nothing else. `GET /v1/builds.json` (no token) serves the
file of the newest patch that has one, so the previous patch's table stays up until the new
patch reaches `fit_at` games. No new tool file is installed; going live is a new
build on the minipc and the owner running the install script.

Tests: hand-built timelines for each definition (undo, first back, a Manamune, a short game),
percentiles and thresholds on small counts, one run over real games checked by hand for two
champions; the service's route and trigger with the fake runner.

## Part 3: review

- `crates/clients/src/history.rs`: `MatchDetail` gains my purchases (time, item), undone ones
  removed when the client's timeline has `ITEM_UNDO`. Still open: no real client timeline is
  saved, so the parser is tested on a hand-extended fixture; if the client sends no undo events,
  an undone purchase counts, which the tips tolerate (it moves a time earlier by seconds).
- `coach::builds` (pure): `Table::parse(json)`, and `review(detail, table, coachless)` giving,
  for my champion and role: my finished items with times beside the `all` and `top` percentiles, my
  first back beside the table's, and tips.
- A tip fires only on these, and the review shows the first two in this order (the quoted lines
  show the wording; their numbers are made up):
  1. Order: my first item, or my second after a listed start, is not in the table's list (under
     5 % and 10 %). "Statikk Shiv first is rare on Jinx bottom; Emerald+ start Kraken Slayer
     (61 %) or Yun Tal (22 %)."
  2. Situation: the game was in a listed situation and I took the overall item, not that
     situation's. "Into magic damage Emerald+ go Maw second (44 %), not Rageblade."
  3. Timing: an item later than the 75th percentile, its own when the table has it, else the
     slot's. "First item at 14:10; Emerald+ finish it between 10:40 and 13:20."
  4. First back: later than the 75th percentile and gold in pocket above the 75th. "First back
     at 6:50 with 1,650 gold; Emerald+ go back between 3:40 and 5:10, most often for Serrated
     Dirk."
- Coachless first: `coachless` is the champion's Coachless build when the app has it. An order
  or situation tip is not shown when the item I took is one of Coachless' options for that
  slot, whatever the crawl says. Timing and first-back tips do not depend on it.
- Summoner's Rift only, not Swiftplay (faster gold; told by queue 480 in the review). No table entry for the pair: no section.
- `src-tauri/src/builds.rs`: downloads `{server_url}/v1/builds.json` at start and every 6
  hours through the disk cache, like the win-probability table; `LOL_COACH_BUILDS_URL` names
  another place. No server address: no table.
- UI: a Build section in `web/src/components/idle/Analysis.svelte`: one row for each item with my
  time and the Emerald+ and Master+ middle half, the first back row, the tips, and one line with
  the table's patch and game count.

Tests: parser on the saved timeline; `coach` tests for each tip and for silence (inside the
middle half, pair missing, Swiftplay) on a small table cut from a real one in `fixtures/coach/`;
the mock state for the section.

## Part 4: in game

- `GameAdvice` gains a pace: the slot of my next finished item and its `all` percentiles, from my
  champion, role and the finished items I hold. Shown as one line on the item card
  (`BuyNext.svelte`): "first item: Emerald+ by 12:00".
- Nothing in game names an item from the crawl: Buy next stays Coachless' (an earlier draft had
  a note where the crawl's usual next item differed; dropped, since Coachless has the larger
  data). The slot is the count of finished items I hold, so no purchase order is needed.
- Silent: no audio, nothing on the overlay.

Tests: `coach` tests for the pace by slot, and for no line without a table entry or in Swiftplay.

## Order of work

Parts 1 to 4 in order; each leaves the app and service working. Part 1 changes what the crawl
stores and should go live first, since the lower band only starts filling then. Parts 3 and 4
need the app rebuilt, which is also still owed for the sync work.

## Not in this design

- Win rates of items or paths, runes, skill order, starting items.
- Tips in ARAM, Mayhem or Swiftplay.
- Bands finer than Emerald+ and Master+ in the tables (the crawl keeps Diamond and Emerald apart
  for the fit report only).
