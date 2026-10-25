# Lane matchup tips (champ select and laning)

Date: 2026-10-25. Status: built.

## Goal

When I play a champion into a lane opponent ("I was Sett vs Zoe and she beat me hard"), show
concrete tips for that matchup in champ select and in game: how the enemy wins, their key spells
and cooldowns, power spikes, what to build and which runes, when to trade, and my own record.

## Decisions (asked with clickable options)

- **Source:** hand-written notes plus computed data.
  - `crates/data/champ_notes.toml`: one entry per champion (all 173), keyed by Data Dragon id:
    lane tags (`level2`, `level6`, `engage`, `poke`, `sustain`), `watch` (spell keys to play
    around), `wins`, `spike`, `trade` (one line each, at most 90 characters, no cooldown numbers).
    Replaces the bot-only `lane_notes.toml`.
  - `crates/data/matchup_notes.toml`: a line for an exact pairing, `[Me] Them = "…"`, written for
    the champions I play (Nautilus, Thresh, Braum, Sett).
  - Data Dragon `champion/<Id>.json`: Q W E R names, icons and cooldowns per rank (same file and
    cache as the enemy R ranks).
  - lolalytics matchup page `/lol/<me>/vs/<them>/build/`: win rate, most played runes, summoners,
    skill order, starting and first items (`summary.pick`).
  - LCU match history: my record against them.
- **Champ select:** one Matchup card for my lane opponent(s); in bot lane both enemy bot laners.
  It replaces the Bot lane card (partner synergy and the build block live in it). The "Your
  history vs them" card keeps only enemies the Matchup card doesn't show.
- **In game:** a laning panel at the top of the threats column with the same cards and R's base
  cooldown at the rank their level allows; after 14:00 it shrinks to one line per opponent (spells
  to watch, trade line) with a "Show tips" toggle.
- **Record:** as my current champion and with any champion, gold at 15:00 in lane games, kills on
  me before 15:00, last three results (newest first).

## Shape

- `model-static`: `ChampNote`, `SpellInfo`, `LaneNotes` (champs + pairs, not exported to TS);
  `MatchupBuild` gains `runes`, `summoners`, `skills`.
- `model-draft`: `MatchupPlan { lane, allies, partner, cards, build, runes }` replaces
  `BotLanePlan`; `MatchupCard { champ, vs_wr, games, tags, note, pair, spells, record }`;
  `SpellCd`; `MatchupMemory` gains `early_kills`, `same_games`, `same_wins`, `last`.
- `model-live`: `GameAdvice.matchup: Vec<MatchupCard>` (model-live now depends on model-draft).
- `coach::lane_matchup`: `card`, `plan` (champ select), `laning` (in game, from `live_extras`).
  Bot lane is "my position or my lane opponent's position is bottom/support"; the second card is
  the other enemy with a bot lane position, bot laner first.
- `src-tauri`: `draft::matchup` fetches `vslane` counters per opposing lane, the matchup page
  against the direct opponent (their bot laner while their support is unknown) and Data Dragon
  spells; `ingame::spells` serves both phases from the `spells` cache.

## Not done

- Ability haste: cooldowns are base values.
- No tracking of when enemy spells were used (Riot policy); the card lists cooldowns only.
- Notes are static text and can go stale after reworks; the test only checks that every
  champion has one and that lines stay short.
