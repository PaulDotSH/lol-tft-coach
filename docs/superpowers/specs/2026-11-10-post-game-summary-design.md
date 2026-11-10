# Post-game summary in plain language

Date: 2026-11-10. Status: draft for review. Idea A2 in `docs/ideas.md`.

## Goal

After a game, the review shows a short written note: three takeaways and one thing
to practise, in plain sentences, built only from facts the review already computed.
Optional and off by default. It never runs during champ select, loading or a game.

Stopping condition: on the flex fixture and on five real reviewed games the note
contains no number, champion or event that is not in the fact sheet, and a rejected
or failed reply leaves the review exactly as it is today.

## What it adds over today

The review already has rule-made `takeaways` and a `focus`. They are correct but
separate lines. The note's job is to connect them ("you were ahead by 1,150 at 14:00
and lost it in two river fights you joined late") and to pick what matters. It adds
no new analysis; if the rules didn't find it, the note can't say it.

## Decisions

| Topic | Decision |
|---|---|
| Model | A local model behind an OpenAI-compatible chat endpoint on 127.0.0.1 (llama.cpp server, Ollama, LM Studio). The app bundles no model and no runtime. |
| Cloud option | Second backend, off unless a key is entered: the Claude Messages API, default model `claude-haiku-5-5`. The fact sheet holds champion names and numbers only: no Riot ids, no summoner names. Open question for the user: ship this backend or keep the app strictly local as A2 says. |
| Input | A fact sheet of numbered lines produced by pure code. The model never sees raw match data. |
| Output | JSON: takeaways and a focus, each citing fact numbers. Checked by code before anything is shown. |
| When | On demand ("Write summary" in the review), or automatically for the newest game when the setting is on, only in the Idle and PostGame phases. |

## Design

### 1. Fact sheet (`crates/coach/src/brief.rs`, pure)

```text
brief::facts(a: &MatchAnalysis, session: Option<&Session>, sd: &StaticData) -> Vec<Fact>
Fact { id: u32, kind: FactKind, good: Option<bool>, text: String }
FactKind { Result, Lane, Deaths, Fights, Objectives, Economy, Vision, Items, Habit }
```

One line per fact, numbers already formatted, 20–40 lines per game:

- result, champion, lane, length, K/D/A, shares (`kill_participation`,
  `damage_share`, `gold_share`);
- laning: `Laning.verdict`, `turned_at`, the first backs;
- every `Takeaway` and the `Focus` text and reason;
- deaths: one line each from `DeathNote` (time, killer, place, gold held, whether
  Buy next was affordable, spike within 60 s, traded), plus `DeathReview.notes`;
- fights: one line each from `Fight` (score, place, presence, died first, what was
  taken after);
- `phases` at 10 and 15, `rich_marks`, `vs_past` rows;
- habits across the session: `SessionRepeat` lines.

Mayhem: augments I took are listed by name. No augment ranks or rates go in.

### 2. Prompt and reply

System text (fixed, versioned as `PROMPT_V`): write for the player in the second
person, three takeaways at most, one focus, each one or two sentences, cite the fact
numbers used, use only numbers and names that appear in the cited facts, no advice
about things the facts don't cover, output JSON only.

```json
{"takeaways": [{"text": "...", "facts": [3, 7], "good": false}],
 "focus": {"text": "...", "facts": [12]}}
```

### 3. Check (`brief::check`, pure)

A takeaway is kept only if all of these hold:

- it cites at least one fact id, and every id exists;
- every number in its text (digits, with `,`, `.`, `:` and `%` normalised) appears in
  one of its cited facts;
- every champion name in its text is in this game's roster, and every item or
  augment name is in the fact sheet;
- it is under 300 characters.

Fewer than two takeaways left, or no valid JSON: the note is dropped and the review
shows nothing new. One retry with the rejected lines named is allowed.

```text
brief::check(reply: &str, facts: &[Fact], roster: &[String], names: &[String]) -> Option<GameNote>
GameNote { takeaways: Vec<NoteLine>, focus: Option<NoteLine>, model: String, prompt_v: u32 }
NoteLine { text: String, facts: Vec<u32>, good: Option<bool> }
```

### 4. Backend (`src-tauri/src/summary.rs`)

- `Settings.summary: SummarySettings { mode: Off | Local | Cloud, url, model, auto }`,
  default Off; `url` defaults to `http://127.0.0.1:8080/v1`. A cloud key is stored
  beside the Coachless tokens under `%APPDATA%\lol-coach`, never in `settings.toml`
  and never logged.
- `#[tauri::command] async fn write_summary(game_id)`: builds the analysis as
  `analyze_game` does, the fact sheet, calls the backend through the existing
  `data::Http` client's reqwest, checks, caches, returns `Option<GameNote>`.
- Guard: refuses unless the phase is Idle or PostGame, and aborts a running request
  when the phase changes (a local model would take CPU or GPU from the game).
- Timeout 60 s local, 30 s cloud. Low temperature. Output capped at 400 tokens.
- Cache: `%LOCALAPPDATA%\lol-coach\summaries\<game_id>.json`, keyed by a hash of the
  fact sheet, `PROMPT_V` and the model name; the review reads it without asking again.

### 5. UI

- Review: a "Coach's note" card above the takeaways when a note exists; each line's
  cited facts on hover; the model name in small print. With the feature on and no
  note yet: a "Write summary" button and a spinner.
- Settings: mode, endpoint, model, "write one after each game", and a "Test" button
  that sends a three-fact sample and shows whether the reply passed the check.

## Errors and gaps

- Endpoint down, timeout, check failed: no card, one debug-log line with the reason.
- Small local models may fail the check often. The Test button and the debug log's
  pass rate tell the user whether their model is good enough; the spec recommends
  none by name.
- The note is in English, like the rest of the UI.

## Testing

- `brief::facts` on `fixtures/clients/match_game_flex.json` + timeline: snapshot of
  the lines (stable order and wording).
- `brief::check`: accepted reply; invented number rejected; champion not in the game
  rejected; unknown fact id rejected; line with no citation rejected; malformed JSON
  gives None; two of three lines surviving still gives a note.
- A recorded good reply and a recorded bad reply as fixtures under `fixtures/summary/`.
- `--ignored` live test against the configured local endpoint.
- Phase guard: `write_summary` refuses in ChampSelect, Loading and InGame.
