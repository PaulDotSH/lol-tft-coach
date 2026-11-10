# Next skill point (in game)

Date: 2026-11-10. Status: draft for review. Idea Q3 in `docs/ideas.md`.

## Goal

While a skill point is unspent, the in-game dashboard says which ability to level
("Level up E"). Otherwise it shows the next one quietly ("Next point: E at 8") with
the max order ("Q > E > W"). It removes a misclick and the habit of holding a point.

Stopping condition: the row shows the right ability on the live fixture and in one
real Rift and one real Mayhem game, and disappears for champions the order can't
describe.

## Scope

- Rift, ARAM and Mayhem. Levels 1–18.
- Dashboard only, silent. Not in the overlay and not spoken (a level-up call every
  minute or two would drown the calls that matter). An overlay line can follow later.
- Several options stay visible: the row always shows the whole max order, not only
  one letter.

## Facts (checked 2026-11-10)

- The lolalytics build page already fetched for `ChampStats` carries, next to
  `skillpriority` (`"QEW"`), a `skillorder` row `{id, n, wr}` whose `id` is a number of
  one digit per level, 1 = Q, 2 = W, 3 = E, 4 = R. Thresh support in
  `fixtures/data/lolalytics-build-thresh-support.html`: `132114131343322` (15 levels,
  7,509 games) under `pick`, and a second one under `win`.
- The matchup page (`fixtures/data/lolalytics-vs-braum-ezreal.html`) has the same
  rows; `parse_matchup_page` reads only `skillpriority` today (`MatchupBuild::skills`).
- The Live Client API gives `activePlayer.abilities.{Q,W,E,R}.abilityLevel` and
  `activePlayer.level` (`fixtures/clients/live_allgamedata.json`: Ahri level 11, Q 5,
  E 2). `clients::live::parse` doesn't read them yet.

## Design

### Data (`crates/data`, `crates/model-static`)

- `PageStats` and `ChampStats` gain `skill_order: String` (one letter per level,
  `"QEWQQRQEQEREEWW"`), `skill_order_games: u32` and `skill_priority: String`, all
  `#[serde(default)]`. Parsed in `parse_build_page` from the `pick` summary (most
  played, the same choice as the matchup page's items and runes).
- `MatchupBuild` gains `skill_order` and `skill_order_games` the same way.
- A digit outside 1–4 or an order shorter than 6 levels gives an empty string.

### Snapshot (`crates/clients`, `crates/model-game`)

- `GameSnapshot.my_abilities: Option<[u32; 4]>` (Q, W, E, R ranks), `#[serde(default)]`,
  from `activePlayer.abilities`. None when the block is missing.

### Decision (`crates/coach/src/skills.rs`, pure)

```text
skills::next(order: &str, have: [u32; 4], level: u32) -> Option<SkillNext>
SkillNext { key: String, unspent: u32, at_level: u32, priority: String, source: String }
```

- `unspent = level − sum(have)`, floored at 0.
- Walk `order` level by level, counting points per ability. The first step whose
  count for its ability exceeds what I have in it is the next point; `at_level` is
  that step's level, or my level + 1 when the point is already due. This keeps
  working when I levelled out of order: it asks for the earliest point of the order I
  lack, and never for an ability I am already ahead in.
- Levels 16–18 (the order has 15): R at 16, then the ability with points left.
- Which order: the matchup page's against my lane opponent when it has at least
  `SKILL_MATCHUP_MIN` games (start at 200, in `consts.rs`), else the build page's.
  `source` names it ("vs Braum, 1.2k games" / "lolalytics 16.21").
- No row at all when: the order is empty, `my_abilities` is None, the sum of ranks
  is greater than my level (champions with a free rank: Karma, Jayce, Elise, Nidalee,
  Udyr-style kits), or my champion has no points to place (Aphelios). These are
  detected from the numbers, not from a champion list.

`GameAdvice.skill: Option<SkillNext>` is filled in `game_advice` (it has `stats`).
The matchup order is passed in through `GameInput` as `Option<&MatchupBuild>`. The
matchup page is fetched in champ select today (`draft::matchup_page`); in game the
orchestrator asks for it the same way against `lane_opponent`, which is served from
the cache after a champ select and fetched once otherwise.

### UI (`web/src/components/ingame/SkillNext.svelte`)

One line under Buy next. Unspent: accent colour, "Level up **E**" plus the order.
Otherwise dim: "Next point: E at 8 · Q > E > W". The source is in the tooltip.

## Errors and gaps

- lolalytics down or a page without the row: no line (Coachless builds don't carry an
  order; the lolalytics page is still fetched for benchmarks, so nothing new fails).
- A champion played off-meta (AP instead of AD) may have a different order than the
  most played one. The source text shows which data it is; no detection.
- The Live API updates about once a second, so the line can lag a level-up briefly.

## Testing

- `parse_build_page` on the Thresh fixture: `skill_order == "QEWQQRQEQEREEWW"`,
  games 7509, priority "QEW". `parse_matchup_page` on the Braum–Ezreal fixture.
- `live::parse` on `live_allgamedata.json`: ranks read, None without the block.
- `skills::next` unit tests: on the order, one point behind, levelled out of order,
  two unspent points, level 16–18, sum of ranks above level (None), empty order (None).
- Mock: `skill` added to the in-game mock fixture; checked in `npm --prefix web run dev`.
- `cargo test --workspace` (regenerates the bindings), `npm --prefix web run check`.
