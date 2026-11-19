# Picks and builds for the lane and for the game (champ select)

Date: 2026-11-18. Status: built (champ select).

## Goal

In champ select, show two answers side by side instead of one blended one:

- **Lane**: against my lane opponent only (in bot lane, both of theirs).
- **Game**: against the whole enemy team, each enemy counted in the lane they play, with
  lolalytics' lane-against-lane numbers (the page behind
  `lolalytics.com/lol/sylas/vs/malphite/build/` is Sylas middle against Malphite top).

Both for counter picks and for the build.

## What is wrong today

- Suggested picks is one list: lane opponent at weight 1.0, every other enemy at 0.3.
- The other enemies' numbers come from the wrong table. The app fetches each enemy's stats in
  *my* lane, so for a mid pick against Malphite top it reads Malphite middle against mid laners.
  Checked on 2026-11-18 (emerald+, 30 days): Malphite middle against Sylas is 43.3% over 2,181
  games; Malphite top against Sylas middle, the real pairing, is 49.2% over 15,848 games.
- The build plan uses my champion's stats over all its games. The enemy team only enters through
  the rules (armor against physical damage, and so on). The lolalytics build against my lane
  opponent is on the opponent's card as a raw path, and nothing uses the pages against the other
  four.
- On a patch's first days the matchup page is nearly empty (Sylas against Malphite: 59 games on
  16.23) and has no fallback, unlike the champion stats.

## Decisions

- **Lane picks**: my lane's tier list ranked by win rate against my lane opponent, shrunk toward
  50% by games as today. In bot lane the mean over their bot laner and their support, each from
  its own lane-against-lane table. No synergy or team composition terms: it answers "who wins
  this lane pairing" and nothing else.
- **Game picks**: the tier list win rate, plus 0.6 (`MATCHUP_BLEND`) times my edge against every
  revealed enemy, plus the synergy and composition terms as before. Each enemy's numbers come
  from its lane-against-lane table (`counter` with `vslane`), and every enemy weighs the same:
  `LANE_OPP_W` and `OTHER_ENEMY_W` are gone. The edge is lolalytics' "delta 2" (my win rate
  against them beyond what both champions' win rates predict), shrunk by games. Edges add up
  over enemies instead of being averaged: an average would count my own win rate five times
  and each matchup a fifth. Against a single enemy this is the old formula.
- **Builds come from the same item engine three ways.** The matchup page has the same per-slot
  item tables (item 1 to 5, boots) as the champion's own build data, so the engine runs on them
  unchanged:
  - **Lane build**: the tables from my page against my direct opponent, and the enemy needs
    (armor, magic resist, anti-heal, …) counted from my lane opponent(s) only. Not shown when the
    page has under 100 games (`MIN_MATCHUP_GAMES`, as the card already requires).
  - **Game build**: the tables from my pages against every revealed enemy in their lane, added
    together with my champion's own tables, and the needs from all enemies as today. A page
    counts for at most `GAME_PAGE_GAMES` games per slot (2,000 to start): its rows are scaled down
    past that, never up, so five well-sampled enemies weigh the same and a thin page weighs what
    its games are worth. My champion's own tables count as one more page under the same cap, which
    keeps the build sane while few enemies are revealed. With no page it is today's build.
- **Thin patch**: the matchup page falls back to the last 30 days when the current patch has
  under 100 games of the pairing (`VS_MIN_GAMES`), a `vslane` table when it has under 3,000
  games over all its rows (`MIN_GAMES`); the fuller answer wins, and the lane build's source
  line says which it is.
- **Requests**: the lane-against-lane counter tables are one small JSON per revealed enemy and are
  fetched on reveal. The pages against the enemies outside my lane are fetched only once my
  champion is locked in (hovering through six champions would otherwise queue two dozen
  400 KB pages); before that the game build uses the pages already there. Everything stays in the
  existing 12 hour caches.
- **Enemy lanes** are the draft report's placement (`coach::draft_lineups`), as the matchup card
  uses now. A wrong guess corrects itself when the placement changes.

## UI

- **Picks**: the "Suggested picks" panel becomes two columns, **Game** and **Lane**, five rows
  each, same row style (icon, name, reason). Lane reasons read "54.2% vs Malphite · 3.1k games";
  the header names the opponent. Bans stay next to them.
- **Tier board**: keeps the lane's tier list, each row still with its win rate against my lane
  opponent. Its "counters to X" columns moved into the Lane column above, so the same ranking
  is not shown twice.
- **Build plan**: two rows in the one panel, **Game · vs their team** and **Lane · vs Malphite**,
  each with start, core items and boots and its own source line. The Lane row appears once the
  page is there.
- The matchup card keeps lolalytics' own most played and highest win rate paths: that is what
  people buy, the Lane row is what the coach would buy.

## Shape

- `data::lolalytics`: `parse_matchup_page` also reads the "Item 2" to "Item 5" tables into
  `MatchupBuild.slots` (`first_items` became `slots[0]`); `matchup` and `counters_vs` get the
  30 day fallback (`MatchupBuild.patch` says which); `Matchup.delta` is the counter row's `d2`.
  Fixture test on `fixtures/data/lolalytics-vs-malphite-sett.html`.
- `coach` (pure):
  - new `vs_build`: `game(mine, pages) -> ChampStats`, the scaled sum of slot and boots rows;
    `lane(mine, page) -> ChampStats`.
  - `champ_select`: `picks` reads the lane-against-lane tables; new `lane_picks`; `build_plan`
    takes the stats and the foes to count needs from.
  - `ChampSelectInput` gains `vs: &[EnemyVs]` (enemy, its lane, its win rates against my lane's
    champions) and `pages: &[&MatchupBuild]`.
- `model-draft`: `ChampSelectAdvice` gains `lane_picks: Vec<PickSuggestion>`, `lane_vs` and
  `lane_build: Option<BuildPlan>`; `BuildPlan` gains `vs: Vec<ChampKey>`; `TierBoard` loses
  `counters` and `others`, and `TierVs` is gone.
- `src-tauri`: `draft::placed`, `enemy_tables` and `my_pages` place the enemies and fetch their
  `vslane` tables and my pages; `orchestrator::champ_select_advice` passes them in;
  `draft::tier_others` is gone. The smoke example prints both pick lists and both builds.
- `web`: `Suggestions.svelte`, `BuildPlan.svelte`, `TierBoard.svelte`, `ChampSelect.svelte`, the
  mock states.

## Tests

- Parser: slots 2 to 5 from the saved page; the 30 day fallback picks the fuller answer.
- `vs_build`: a page over the cap is scaled down, one under it is not; an item only one page has
  keeps its row; no pages gives back my own tables.
- Picks: lolalytics' delta is taken when the row has one; edges of two enemies add up with equal
  weight; lane picks ignore the other enemies; bot lane uses both. Which table reaches the coach
  is the app's side (`draft::enemy_tables`) and is covered by the smoke example only.
- Build: the lane build counts needs from the lane opponent only; hidden under 100 games.
- `cargo test --workspace`, `npm --prefix web run check`, `npm --prefix web test`, and the smoke
  example against live data.

## Not in this change

- In game: Buy next still uses my champion's general stats. Feeding it the game tables (all five
  enemies and their lanes are known there) is the natural next step and reuses `vs_build`.
- Runes: the page against my lane opponent is already an option; no game-wide rune page.
- ARAM and Mayhem: no lanes, nothing changes.
