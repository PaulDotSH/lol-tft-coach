# Post-game summary in plain language

Date: 2026-11-10. Status: built (rules only, see the end). Idea A2 in `docs/ideas.md`.

## Goal

After a game, the review shows a short written note: up to three takeaways and one
thing to practise, in plain sentences, built only from facts the review already
computed. It never runs during champ select, loading or a game.

Stopping condition: on the flex fixture and on the real reviewed games on disk the
note contains no number, champion, item or augment that is not in the facts it cites.

## What it adds over today

The review already has rule-made `takeaways` and a `focus`. They are correct but
separate lines. The note's job is to connect them ("you won your lane, +461 at
14:00, and kept it going in the fights at 21:38 and 23:18") and to pick what
matters. It adds no new analysis; if the rules didn't find it, the note can't say it.

## Decisions

| Topic | Decision |
|---|---|
| Writer | Rules only: no model, no endpoint, no network, no keys. Cheap enough to run with every review. |
| Input | A fact sheet of numbered lines produced by pure code from `MatchAnalysis` (and the session). |
| Output | `GameNote`: takeaways and a focus, each line citing fact ids. Every line passes `brief::check`. |
| When | Built with the review (`analyze_game`) when the setting is on and the phase is Idle or PostGame. No cache. |
| Setting | `Settings.game_note: bool`, default on. |

## Design

### 1. Fact sheet (`crates/coach/src/brief.rs`, pure)

```text
brief::facts(a: &MatchAnalysis, session: Option<&Session>, sd: &StaticData, augments: &HashMap<AugmentId, String>) -> Vec<Fact>
Fact { id: u32, kind: FactKind, good: Option<bool>, text: String }
FactKind { Result, Lane, Takeaway, Focus, Deaths, Fights, Objectives, Economy, Vision, Items, Habit }
```

One line per fact, numbers already formatted, about 40 lines per game, in this order:

- result, champion, lane, length, K/D/A; shares (`kill_participation`,
  `damage_share`, `gold_share`); Mayhem augments I took, by name (no ranks or rates);
- laning: both sides, `Laning.verdict` (which carries `turned_at`), the first backs;
- `phases` at 10 and 15; the team gold peak and low; `rich_marks`; vision and wards;
- every `Takeaway`, and the `Focus` text and reason;
- deaths: one line each from `DeathNote` (time, place, killer and assisters, gold
  held, Buy next affordable, spikes, traded), plus `DeathReview.notes`;
- fights: one line each from `Fight` (time, place, kills each way, presence, died
  first, what was taken after);
- objectives each way, and each epic monster they took with where I was;
- `vs_past` rows; habits: `SessionRepeat` lines when this game is in the session.

### 2. Composer (`brief::compose`, `brief::note`, pure)

Rules make candidate lines from structured data, with the same formatters as the
facts, and cite the facts they use. Ranked by score (constants in `consts.rs`):

| Score | Rule |
|---|---|
| habit 7 | A session habit and this game's instance: died before 10:00, behind at 15:00, no Control Wards (with wards vs the opponent), 6+ deaths. |
| lead 6 | Loss: team lead of 2,500+ and the lost fights after it; or a won lane and the lost fights after 14:00. |
| pattern 4 | Deaths holding 1,000+ gold (Buy next affordable, gold sat on); 2+ early deaths outside won fights; died first in 2+ lost fights; both ward takeaways in one line. |
| away 3 | Epic monsters they took while I was elsewhere. |
| good 2 | Win: a comeback from 2,500 behind and the fights won after; a won lane and the fights won after. |
| plain 1 | Each review takeaway not said by a chosen line (the Mayhem damage line names the augments). |

At most three takeaways, one of them good when a good one exists; one line per topic;
at most three instances per line (counts in words, "among them" when cut). The focus
is the review's focus plus the best line on its topic (that line then leaves the
takeaways). Fewer than two takeaways: no note.

### 3. Check (`brief::check`, `brief::passes`, pure)

A line passes when it cites at least one fact, every id exists, it is under 300
characters, every number in it (digits with `,` `.` `:`; thousands separators
dropped) is in a cited fact, and every champion, item or augment name in it appears
in a cited fact. `check` drops failing lines and returns None under two takeaways.
`note` = `check(compose)`; tests prove `compose` emits nothing that fails.

```text
GameNote { takeaways: Vec<NoteLine>, focus: Option<NoteLine>, facts: Vec<Fact> /* the cited ones */, rules_v: u32 }
NoteLine { text: String, facts: Vec<u32>, good: Option<bool> }
```

### 4. App

`analyze_game` sets `MatchAnalysis.note` after `compare_past`, with the session from
the trends and Mayhem augment names when cached.

### 5. UI

Review: a "Coach's note" card at the top of the left column, above the focus and
the takeaways; hovering a line shows its cited facts. Settings: "Coach's note in game
reviews" toggle. Mock: `web/src/mock/analysis.json` carries the composer's note.

## Testing

- `brief::facts` on `fixtures/clients/match_game_flex.json` + timeline: snapshot in
  `fixtures/coach/brief_flex_facts.txt` (`BLESS=1` rewrites it).
- Exact notes on the flex game with and without a session; a lead lost and held gold
  with Buy next; variants (loss, comeback, each focus, Mayhem, sessions) all pass the
  check; a remake or quiet game gives no note.
- `check`: invented number, champion or item not in the cited facts, unknown id, no
  citation, too long rejected; two of three surviving still gives a note.
- The mock note equals the composer's on the mock analysis.
- `--ignored real_games_on_disk`: games saved by debug mode under
  `%LOCALAPPDATA%\lol-coach\debug`.

## Implementation status (2026-11-10)

Built: `crates/coach/src/brief.rs` (facts, compose, check, note), model types
`Fact`/`FactKind`/`NoteLine`/`GameNote` and `MatchAnalysis.note`, `Settings.game_note`,
`analyze_game` wiring, `NoteCard.svelte`, the settings toggle, mock data, tests.

Deviations from the reviewed draft:

- User decision: no model at all. Dropped the local and cloud backends, prompt, JSON
  reply parsing, retry, API keys, `summary.rs`, the cache, the Test button and the
  "Write summary" button. `GameNote` has `rules_v` instead of `model`/`prompt_v`.
- `FactKind` gained `Takeaway` and `Focus`. `facts` and `note` take the Mayhem
  augment names (they are not in `StaticData`); `note` takes the analysis rather than
  the facts, since the rules work on structured data and cite facts by source.
- The check requires names to be in the cited facts (stricter than "in the roster").
- Session habits are matched on `SessionRepeat.text`, now shared constants in
  `session.rs`; the focus topic is matched on `Focus.goal` or its text.
- `coach` gained `clients` and `data` as dev-dependencies (workspace crates) to
  parse the flex fixture and Data Dragon fixtures in tests.
- Five games checked: the flex fixture and the four games with a timeline in the
  debug dumps on disk; all lines pass.

Open: thresholds and scores are first guesses; no habit line for session goals; the
card was checked by `svelte-check` only, not looked at in the running UI.
