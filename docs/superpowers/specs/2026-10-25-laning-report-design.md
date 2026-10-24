# Laning report (game review)

Date: 2026-10-25. Status: draft for review.

## Goal

The game review gets a "Laning" section for the first 14 minutes: my duo against
their bot lane, minute by minute. It shows when the lane was won or lost and why
(kills, levels, CS, backs), so the user can see the turning point instead of only the
5/10/15 snapshot.

The user picked "Duo vs duo, minute by minute".

## Scope

- Rift games with a timeline. ARAM/Mayhem have no lanes: no section.
- Bot lane (I'm support or ADC): my duo vs their duo. Other lanes: me vs my lane
  opponent (same view, one player per side), so the section is never empty on Rift.
- Minutes 1–14 (plates fall at 14:00; laning is over by then).
- Replaces the existing by-minute table (`Phases.svelte`, 5/10/15) in the review,
  since the laning report covers the same numbers per minute. Minute 15 stays in the
  takeaways ("Ahead of Thresh by 1152 gold at 15:00").

## What it shows

1. **Verdict line**, from the duo gold difference at 14:00:
   "Won lane: +1,150 gold at 14:00, ahead from 6:00" / "Even lane" (within ±300) /
   "Lost lane: −900 gold at 14:00, behind from 3:00 (first blood)". "Ahead from" is the
   first minute from which the difference stayed above +300 (below −300 for behind).
2. **Gold difference chart**, minutes 1–14, duo vs duo (totalGold sums), the same
   chart style as `GoldChart.svelte`, with kill markers (ours green, theirs red) at
   their minute.
3. **Per-minute rows** (compact table, every minute 1–14 or folded to every 2 minutes
   on narrow screens): level me / them (support vs support, or me vs opponent), CS
   duo vs duo, XP difference.
4. **Lane events** before 14:00 among the lane's players: kills, deaths and assists
   with the clock and the place (existing `MapZone` names): "3:12 Ezreal killed you
   in bot lane (Nautilus assist)".
5. **First backs**, estimated: a minute where a player is in their own base zone and
   their current gold dropped by 300+ since the previous frame. Shown as "You backed
   at ~6:00 (spent ~1,100)", "Their ADC backed at ~5:00". Labelled "about": frames are
   one minute apart.

## Data

All from what `history::parse_match` already reads: per-minute frames
(`totalGold`, `currentGold`, `level`, `xp`, `minionsKilled`, `jungleMinionsKilled`,
`position`) and CHAMPION_KILL events with positions. Lane assignment reuses the
inferred lanes (positions + permutation fix) already used by the review. No new LCU
calls.

```text
MatchAnalysis.laning: Option<Laning>
Laning {
  allies: Vec<ChampKey>, enemies: Vec<ChampKey>,   // 1–2 each
  verdict: String,
  turned_at: Option<u32>,                          // minute it stayed ahead/behind
  minutes: Vec<LaneMinute>,                        // 1..=14 (fewer in short games)
  events: Vec<LaneEvent>,
  backs: Vec<LaneBack>,
}
LaneMinute { minute, gold_diff, xp_diff, cs_ours, cs_theirs, my_level, opp_level }
LaneEvent  { time, killer: Option<ChampKey>, victim: ChampKey, assists: Vec<ChampKey>,
             ours: bool, place: Option<String> }
LaneBack   { champ, ally: bool, minute, spent }
```

`coach::laning::laning(detail: &MatchDetail, me) -> Option<Laning>` is pure; the
review calls it next to the existing phases code (`analysis.rs`).

## Errors and gaps

- No timeline, or my lane opponent can't be identified: section hidden.
- Remakes and games under 10 minutes: section hidden.
- Backs need a base-zone position on a frame; a back between frames that ends before
  the next frame may be missed. The UI says "estimated".

## Testing

- `laning` tests on the anonymized flex fixture already used by the review
  (`fixtures/clients/match_game_flex.json` + timeline): duo pairing, verdict text,
  turned-at minute, events filtered to the lane, a back detected from a gold drop in
  base.
- Unit tests for the verdict thresholds and the "stayed ahead" rule.
- `web/src/mock/analysis.json` regenerated with the `review` example; section checked
  in the browser mock; then a live check on the user's next game review.
