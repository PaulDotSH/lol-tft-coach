# Bot lane 2v2 plan (champ select)

Date: 2026-10-25. Status: draft for review.

## Goal

When I play support or ADC in Summoner's Rift, champ select shows a short plan for
the bot lane 2v2: how my lane matches up in numbers, when their lane wants to
fight, how I did against them before, and what to start with. It answers
"how do I play this lane?" before loading in, in the 30–60 s I have after locking.

The user picked all four parts: matchup numbers, their all-in windows, my past
games vs them, what to buy first.

## Scope

- Rift draft queues only (ranked solo/flex, normal draft), my lane support or bottom.
- Shown once my champion is hovered or locked and at least one enemy bot laner is
  revealed. Parts fill in as enemies are revealed.
- Read-only, from data already allowed: lolalytics aggregates, my own history, and
  hand-written static notes. Nothing about the enemy players themselves (Riot rule:
  no de-anonymizing, no lobby scouting).

Out of scope: other lanes (the existing lane matchup line covers them), in-game lane
advice, ARAM/Mayhem.

## Who is their bot lane

Champ select hides enemy positions. The existing `DraftReport.lanes` already assigns
enemy champions to lanes from lolalytics lane shares; the plan takes its `Bottom`
and `Support` entries. When the assignment is uncertain (a flex pick such as Seraphine
or Swain), the part is still shown, marked "likely".

## Parts

### 1. Matchup numbers

- My win rate vs their ADC and vs their support, from lolalytics `ep=counter` with
  `lane=<my lane>&vslane=<their lane>`. The probe on 2026-10-25 confirmed `vslane=bottom`
  returns support-vs-ADC rows (Braum support: 37 ADC rows, e.g. vs Ezreal 57.97 % over
  1,908 games).
- My duo's synergy with my ADC (or support): the existing `DraftDuo` delta from
  `build-team`.
- Shown as three rows: "vs Ezreal 58 % (1.9k games)", "vs Nautilus 49 % (3.1k)",
  "with Jinx +1.8 pts (2.4k)". Rows under 100 games are greyed as "few games".

### 2. Their all-in windows

- New file `crates/data/lane_notes.toml`, hand-written, about 40 bot-lane champions
  (common supports and ADCs). Per champion: `level2` (strong level-2 all-in), `level6`
  (ult turns the lane), `engage` (hard engage support), `poke`, `sustain`, and one
  short `note` (≤ 80 chars), e.g. Nautilus: level2, engage, note "Hook + passive root:
  stand behind minions, punish a missed Q".
- The plan turns their two champions' tags into 1–3 lines: "Strong level 2: don't
  fight their first two waves unless you hit 2 first", "Level 6 all-in (Nautilus R)".
- A champion missing from the file gets no line, never a guess.
- This is static data available before the game, which Riot allows.

### 3. My past games vs them

- Reuses `MatchupMemory` (already in `ChampSelectAdvice.memory`), filtered to their
  bot laners: "vs Nautilus: 2–1, killed you 4× in 3 games, +350 gold at 15:00".

### 4. What to buy first

- The lolalytics matchup page `lolalytics.com/lol/<me>/vs/<them>/build/?lane=…&vslane=…`
  carries "Starting Items" and "Early Items (10 min)" for that matchup in the same Qwik
  state blob the build page parser (`data::qwik`, `parse_build_page`) already reads.
- Plan: start items vs their ADC (the lane's damage), first-back items as a second row,
  each with win rate and games. Fall back to the general `earlySet` when the matchup has
  under 100 games.
- Spike risk: the vs page's blob layout. First implementation step is a fixture
  (`fixtures/data/lolalytics-vs-braum-ezreal.html`) and a parser test; if the layout
  proves unreliable, this part ships without matchup items (general early set only).

## Data flow

- `data::lolalytics`: `counters_vs(champ, lane, vs_lane)` (same parser as counters),
  `matchup_page(champ, vs, lane, vs_lane)` → starting/early sets.
- Fetched by the orchestrator in champ select, cached per (champ, lane, vs champ,
  vs lane, patch) like other lolalytics calls; fetched once enemies appear.
- `coach::bot_lane::plan(...) -> Option<BotLanePlan>` (pure): inputs are the draft
  lanes, counters, duo synergy, memory, lane notes, matchup sets.
- `ChampSelectAdvice.bot_lane: Option<BotLanePlan>`; UI
  `champselect/BotLane.svelte`, placed above the build plan when present.

```text
BotLanePlan {
  ally_adc, ally_support, enemy_adc, enemy_support: Option<ChampKey>,
  likely: bool,                      // enemy lanes guessed with low confidence
  numbers: Vec<MatchupLine>,         // vs each enemy + duo synergy
  windows: Vec<String>,              // from lane notes
  memory: Vec<MatchupMemory>,        // their bot laners only
  start: Option<ItemSetLine>,        // starting items vs this lane
  first_back: Option<ItemSetLine>,
}
```

## Errors and gaps

- lolalytics down: numbers and items hidden, notes and memory still shown.
- No enemy bot laner revealed yet: card shows "Waiting for their bot lane".
- Unknown champion in notes: no window line.

## Testing

- Parser tests for `vslane` counters (saved JSON) and the vs page (saved HTML).
- `bot_lane::plan` tests: support vs ADC+support, ADC vs both, one enemy revealed,
  flex pick marked likely, few-games greying, notes merged into windows lines.
- Mock state `champselect rift` gains a `bot_lane` example; checked in the browser.
- Live check in the user's next support game.
