# LoL Coach: ideas for future versions

Date: 2026-10-22. Builds on `docs/superpowers/specs/2026-10-22-lol-coach-design.md` (v1).
Every idea below keeps the v1 rules: read-only, second monitor, no overlay, silent,
data only from LCU, the Live Client Data API, lolalytics, u.gg bigbrain JSON, Data
Dragon / CommunityDragon and Windows OCR of our own screen.

Legend: **Policy** none / low / high. **Effort** S (days), M (1–2 weeks), L (more).

---

## 1. What Riot allows (the rules every idea is checked against)

| Rule | Source |
|---|---|
| "Products must not use or incorporate information not present in the game client that would give players a competitive edge". Riot's own example is **tracking enemy ultimate cooldowns**. | [Riot LoL docs, Game Integrity][riot-lol] |
| Unapproved: "Products may not provide any game-session-specific information that would be previously unknown to the player." | [Riot LoL game policy][riot-lol], [support mirror][riot-lol-policy] |
| Unapproved: "**Products cannot display win rates for Augments** or Arena Mode items." Applies to all sites, apps and overlays. | same |
| Unapproved: "Apps that dictate player decisions." General policy: products "may highlight decisions that are important and give multiple choices". | [riot-lol], [General policies][riot-general] |
| No MMR/ELO calculators or alternatives to the ranked ladder. | [riot-general] |
| "Products cannot de-anonymize players who cannot reasonably be identified from visible information" (so: nothing hidden in champ select). | [riot-general] |
| Approved examples: overlays with "static data that is available prior to the game", "Overlays that let you track your own data or known information", "Training tools that allow players to view their own match histories and aggregate stats". | [riot-lol], [riot-lol-policy] |
| Riot objects to exposing "information that's intentionally obfuscated"; its 2014 timers ruling: timers only a third-party app gives you are an unfair advantage. | [Riot support: third-party apps][riot-3p], [2014 red posts][s20-2014] |
| From **6 Oct 2026** Vanguard blocks game-memory access by unknown apps; approved tools get a grace period to **April 2027**, then official APIs only. | [Riot support: memory access][riot-mem] |
| LCU is "not officially supported" for third parties: no uptime or change guarantees. | [riot-lol] |

**What "information not available in game" means in practice.** The test: could
a player with perfect attention see or know it in their own client at that
moment? Allowed: enemy items, level, K/D/A, CS, summoner-spell *names*, death
timers on the scoreboard, announced objective kills, all static game data
(base cooldowns, item stats). Not allowed: enemy gold, enemy ability or
summoner cooldown *state*, anything in fog of war, unseen jungle camps, names
hidden in champ select, augment win rates. Grey: timers derived from announced
events (dragon/baron respawn). These are widely shipped (iTero, Porofessor,
OP.GG) and v1 already has them.

**Consequences for v1 (worth fixing before v2):**
- Mayhem augment advice must show **ranks/tiers only, never win-rate numbers**,
  even if the u.gg JSON carries them.
- Show the three offered augments ranked with a reason, not a single "PICK"
  label. "Buy next + alternatives" already fits the "multiple choices" rule.
- Our read-only, API-only design is now a real advantage: memory-reading
  competitors lose features by April 2027.

## 2. What the others offer

| Tool | Notable features | Gap we can fill |
|---|---|---|
| Porofessor (Overwolf) | Live lobby stats and player tags, ban/counter tips, rune/build import, in-game jungle/inhib timers, gold diff, build overlay, **spell cooldown timers** [porofessor] | Player-focused scouting rather than coaching; overlay only |
| Blitz.gg | Overlays: build, benchmarking (live performance vs metrics), **ARAM health relic timers**, **Mayhem augment tier overlay** on the pick screen, post-match analysis [blitz] | Augment tiers are static per champ; nothing ties augments to items |
| Mobalytics | Live Companion: pre-game, overlay, post-game; GPI "strengths/weaknesses" profile; Mayhem builds from "ARAM data + rating-based augment data" because Riot has no Mayhem API data [moba-guide], [moba-gpi], [moba-mayhem] | GPI is generic and reported as clunky; no live, threat-aware build |
| U.GG app | Stand-alone desktop app (1.9 M+ downloads), builds/tiers; Mayhem augment tier lists on the site (bigbrain JSON) [ugg] | Same data we already read |
| OP.GG desktop | Auto runes/spells; overlays: jungle timers + path, inhib timers, ARAM health timers, **augment tiers**, live skill vs tier average, item build, gold/item diff, **healing-item tracker**, ward count, manual enemy spell/ability cooldown sharing, AI voice agent [opgg-lol], [opgg-status] | Busy overlay; no "why"; nothing on second monitor |
| iTero (GIANTX) | Drafting Coach (best pick for the draft), Macro Coach (loss patterns from recent games), lobby scouting, comp-based rune/summoner recs, objective timers, 500-stat account analyser [itero], [itero-eg], [giantx] | Rift-only; no ARAM/Mayhem; no live item advice |
| YOUR.GG | Mayhem in-game overlay with augment tier list [yourgg] | Same |
| lolalytics site | By-slot item sets, boots, starting/10-min items, skill order, runes, spells, counters by lane, synergy, filters by level/keystone, objective stats, per-champ averages [lola] | Static pages; we combine them with live state |

Nobody combines **live enemy state + Mayhem augments + items** in one place.
That is this app's niche.

---

## 3. Quick wins (S, data v1 already reads)

### Q1. Shopping mode (ARAM first)
When you are dead or on the fountain, the in-game dashboard switches to a big
shopping list: exactly what to buy with your current gold (full items, then
components toward Buy next), your respawn countdown, and the leftover gold.
In ARAM you can only shop while dead, so this is when you look at the second
monitor anyway.
- **Why:** turns the 10 seconds of death time into a correct purchase.
- **Data:** Live API `activePlayer.currentGold`, own `isDead`/`respawnTimer`; Data Dragon `from`/`into` costs. Trivial.
- **Policy:** none · **Effort:** S

### Q2. Gold ETA and back planner (Rift)
Estimate gold income from `currentGold` deltas (ignore drops from spending)
and show "Zhonya's in ~45 s" or "back at 1300 g: Seeker's + Boots".
- **Why:** better back timing is one of the biggest silent mistakes.
- **Data:** Live API gold over time; item costs. Rough but useful.
- **Policy:** none · **Effort:** S

### Q3. Next skill point
Show which ability to level next, from lolalytics' skill order compared with
`activePlayer.abilities[].abilityLevel`.
- **Why:** cheap, removes a common misclick; pulls the v2 "skill order" item forward.
- **Data:** lolalytics build page (skill order is there [lola]; endpoint or Qwik blob is a **spike**); Live API.
- **Policy:** none · **Effort:** S

### Q4. Mode rule overrides (Mayhem correctness)
A `modes.toml` that changes item scoring per mode. Mayhem: Combo Breaker gives
anti-CC for free, so lower the tenacity weight; crit above 100 % converts to AD/AP,
so allow crit overcap for crit carries; attack-speed cap 5.0; ranged champions
get no bonus resists; health relics heal 22 %. Add the Mayhem-only legendaries
(Atma's Reckoning, Rite of Ruin, Sword of Blossoming Dawn) and the Stat Anvil
[wiki-mayhem].
- **Why:** without this, Mayhem advice is generic ARAM advice and sometimes wrong (e.g. Mercs for tenacity).
- **Data:** hand-written table, checked each patch; Data Dragon/CommunityDragon for item availability.
- **Policy:** none · **Effort:** S

### Q5. Level spikes and static ult cooldowns on the threat board
Tag enemies "R online" at 6/11/16 (their level is visible) and show the base
ult cooldown for their rank from static data ("Malphite R 130/105/80 s, rank 1").
Never record when an ult was cast.
- **Why:** gives most of the value people want from ult timers, legally (see R1).
- **Data:** Live API `level`; Data Dragon `champion/<x>.json` `spells[3].cooldown`.
- **Policy:** low (static data + visible level) · **Effort:** S

### Q6. Death windows
A line under the threat board: "3 enemies dead, next back in 18 s"
(Rift: objective/push window; ARAM: push window). It states the fact; it does
not say "go Baron".
- **Why:** players forget to look at the scoreboard; the information is already on it.
- **Data:** Live API `playerlist[].isDead`/`respawnTimer` (fields confirmed in [ow-gep]).
- **Policy:** low · **Effort:** S

### Q7. Enemy summoner spells as context
Show enemy spell *names* on threat rows and let them feed the needs vector:
an enemy carry with Cleanse makes CC-chain items worth less; Ignite on a
bruiser raises anti-heal weight. No cooldowns.
- **Data:** Live API `summonerSpells` (also on the loading screen).
- **Policy:** low · **Effort:** S

### Q8. Dragon soul tracker (Rift)
Dragons per team, soul point, soul type from `mapTerrain`, elder after soul.
- **Data:** `DragonKill` events (`DragonType`), `gameData.mapTerrain`.
- **Policy:** none · **Effort:** S

### Q9. "Died holding gold" (Rift recap)
Recap line: "died 3× with more than 1000 g unspent". Uses the recorder's
snapshots.
- **Policy:** none · **Effort:** S

---

## 4. v2 (M)

### V1. Mayhem reroll advisor (per card)
Each card on an augment screen can be rerolled once, and a Golden Reroll moves
an offer up one rarity [wiki-mayhem]. For each OCR'd card, compute its percentile in the
champion's u.gg list for that rarity, then the expected gain of rerolling it:
the chance a random unseen augment of that rarity ranks higher (pool size from
the manifest, minus augments already offered or taken this game). Output: all
three cards ranked, with "keep / reroll" per card and a one-line reason
("Card 2 is in your bottom 30 %; a reroll beats it 70 % of the time").
- **Why:** reroll decisions are where Mayhem players lose the most value, and no competitor advises on them.
- **Data:** u.gg per-champion rarity lists (ranks only); OCR bounding boxes give card position.
- **Policy:** low. Ranks/percentiles only, never augment win rates; always several options.
- **Effort:** M

### V2. Augment-aware item scoring (Mayhem's main feature)
When your picked augments are known (V3), add their tags to the needs vector
and to item candidates. Tags come from a curated `augments.toml` (on-hit, crit,
attack speed, ability haste, AP ratio, healing/shield power, tank scaling,
mana, range, "grants tenacity") plus keyword parsing of augment descriptions
(**spike:** description source: CommunityDragon or the bigbrain manifest).
Examples: an on-hit augment raises Nashor's/BotRK/Terminus; a haste augment
lowers Cosmic Drive and raises AP/damage items; a tenacity augment removes
Mercs from boots; quest augments that reward an item keep a slot free and skip
duplicate items. Also the reverse in the augment ranking: an augment that
matches your already-built items gets a small tie-break bonus (shown as a
reason, not a win rate).
- **Why:** "augment + item combos" is what the user asked for and no tool does it.
- **Data:** V3 + u.gg manifest + Data Dragon item tags; hand curation is the bulk of the work.
- **Policy:** low · **Effort:** M

Note: augment **sets/set bonuses** (added 26.3 [patch-26-3]) and traits were
removed in 26.12 [mayhem-26-12]. Keep a data hook (`set_id` in `augments.toml`) in
case they return; don't build UI for them now.

### V3. Picked-augment detection
v1 OCR sees the offer, not the choice. Options, in order of preference:
(a) when the offer screen closes, diff `activePlayer.championStats` (stat
augments change armor/AP/haste visibly); (b) template-match the HUD augment
icons against bigbrain `.webp` icons; (c) last resort, a one-click "I took
#2" button when dead (breaks "no clicks mid-game" only while shopping).
**Spike:** check whether Live API exposes augments in Mayhem at all (no
public sample shows a field [ow-gep]).
- **Policy:** none (own screen, own data) · **Effort:** M

### V4. Mayhem late-game gold sinks
After six items: Stat Anvils (level 9+, each with a free Cappa Juice [wiki-mayhem]), elixirs,
sell boots when augments cover move speed, and which slot to swap given the
current threats.
- **Data:** Q4 table + item scoring.
- **Policy:** none · **Effort:** S–M

### V5. Resist-aware penetration and your own EHP split
Estimate each enemy's armor and MR: base + growth × (level − 1) + item stats
(+ Mayhem bonus resists), all visible or static. Use it to choose lethality vs
% armor pen and flat vs % magic pen. Also show your own effective HP against
physical vs magic from your `championStats`, next to the enemy damage split:
"EHP vs magic 1900 / vs physical 3100; enemy damage 65 % magic".
- **Why:** makes defensive and pen choices numeric instead of guessed.
- **Data:** Data Dragon champion stats + items; Live API `activePlayer.championStats`.
- **Policy:** low · **Effort:** M

### V6. Enemy item spikes
On each enemy inventory change, tag completed legendaries ("Zed: 2 items, core
done") and compare item count/gold to that champion's typical power curve
(lolalytics item-set win rate by slot). Feeds the threat score.
- **Policy:** low (visible inventory + aggregate stats) · **Effort:** M

### V7. Matchup cheat sheet from static data
For the lane opponent (Rift) or the whole enemy team (ARAM), computed facts:
range vs your attack range, hard-CC count and which spells, dashes, spell
shields, base cooldowns of their key spells, plus lolalytics `vsWr`. Example: "Q
range 1050 vs your AA 525 · 2 hard CC (Q, R) · no dash". No hand-written text needed.
- **Why:** "matchup tips" without maintaining 170×170 notes.
- **Data:** CommunityDragon champion bin / Data Dragon spells; lolalytics `counter`.
- **Policy:** low (static, available before the game) · **Effort:** M

### V8. Team composition panel (champ select, including the ARAM bench)
Damage profile (AD/AP/true), frontline count, engage/peel, CC total. Point out
gaps ("no frontline") and rank pick/bench options by how well they fill them,
shown as options with reasons. In ARAM this improves v1's bench ranking: the
best Mayhem-tier champion is not always the best for *this* team.
- **Data:** champion tags + `traits.toml`; lolalytics/u.gg tiers.
- **Policy:** low (own team only; enemies hidden in ARAM) · **Effort:** M

### V9. Runes (display only, Rift)
lolalytics runes for champ/lane, nudged by enemy comp (e.g. Bone Plating vs
burst lanes). Display only, no import (see R7). Mayhem has no runes.
- **Policy:** none · **Effort:** M

### V10. Death review (recap)
For each death: time, killer and assisters, item-gold gap vs killer, whether
the killer was top of the threat board, and what Buy next said at that moment
vs what you owned. Add an "enemy R online" marker (Q5) so a pattern like "died
3× within 60 s of Malphite hitting 6" is visible.
- **Why:** the most actionable self-coaching, built from data v1 already records.
- **Data:** kill events + recorder snapshots + advice timeline.
- **Policy:** none · **Effort:** M

### V11. Personal trends and goals
Local history of recaps (plus backfill from LCU own match history,
`/lol-match-history/v1/products/lol/current-summoner/matches`): CS/min at 10,
deaths before 14:00, vision/min, unspent gold, "followed Buy next" % vs result.
Pick 1–2 goals; the live self-coach row shows progress against them, silently.
Mayhem slice: games and average damage per augment taken, **no augment win
rates** (Riot rule applies to personal stats too).
- **Policy:** none (explicitly approved use case) · **Effort:** M

---

## 5. Ambitious (L)

### A1. Item what-if simulator
Damage and EHP calculator against the current enemy team: your stats, enemy
resists from V5, item stats and hand-modelled passives for the ~60 most-built
items. "Rabadon's vs Void Staff: +11 % to Malphite, +4 % to team average."
Makes situational scoring physical instead of tag-based.
- **Policy:** low · **Effort:** L (passive modelling, patch upkeep)

### A2. Local LLM post-game summary
Optional, off by default: a local llama.cpp model turns the structured recap
(V10/V11 facts only) into three plain takeaways. Nothing leaves the PC; no
in-game use.
- **Policy:** none · **Effort:** L (prompting + checking it doesn't invent facts)

### A3. Enemy augments from the scoreboard (Mayhem)
If the Tab scoreboard shows every player's augments (**spike:** confirm it does
in the client), OCR/icon-match it while the user holds Tab, and feed enemy
augments into threat and needs ("Sion took a tank-scaling prismatic: raise %HP
damage").
- **Policy:** low only if confirmed visible in the client; high otherwise · **Effort:** L

### A4. Gold/level timeline vs lane opponent
Post-game chart of item-gold and level vs your lane opponent (or lobby
average in ARAM) with death/objective markers, from recorder snapshots. Not a
damage graph (still out of scope).
- **Policy:** none · **Effort:** M–L

### A5. Draft score (Rift)
An iTero-style comp estimate from lolalytics `counter` and `build-team` matrices,
shown as a breakdown next to the counter-pick list, never as "pick X".
- **Policy:** low · **Effort:** L (many requests; needs strict caching to stay polite)

---

## 6. Rejected

### R1. Enemy ult timers from sound detection (asked by the user)
**Rejected: policy high.** Riot's integrity rule names this exact case:
products "must not use or incorporate information not present in the game
client", e.g. "tracking enemy ultimate cooldowns" [riot-lol]. Hearing the cast
does not make the timer "known information": the client never shows an enemy
ult's remaining cooldown, and Riot has said since 2014 that timers only a
third-party app gives you are an unfair advantage [s20-2014]. OP.GG's
manual cooldown sharing is player-entered and still grey; ours would be
automatic. The account risk falls on the user, not on us.
It is also weak technically: WASAPI loopback capture is easy, but ult sounds
depend on skin, distance and fog, nine other champions are in the mix, and
global ults sound alike across champions. A classifier would need labelled clips per
champion × skin and would still miss casts in fog.

**Allowed alternatives with similar value:**
- Q5: "R online" at level 6/11/16 + static base cooldown for their rank.
- Q6: dead enemies can't ult: death windows from the scoreboard data.
- V10: "your deaths vs enemy level-6/ult spikes" in the recap, so you learn to respect the spike.
- V7: which enemies have game-changing ults (hard CC, global, execute), from static data, before the fight.
- V11: a goal like "no deaths within 60 s of the enemy laner hitting 6".

### R2. Enemy summoner-spell timers (manual or automatic)
Same rule as R1. Manual timers would also need clicks mid-game. **Policy: high.**

### R3. Enemy jungle camp timers, jungle tracking, fog-of-war positions
No API data (only epic-monster kills are announced) and it is hidden
information by definition. **Policy: high.**

### R4. ARAM health relic timers
Blitz and OP.GG ship them [blitz], [opgg-lol], but the Live API has no relic
events. It would need memory reading (blocked from Oct 2026 [riot-mem]) or
minimap OCR (fragile), and relics taken in fog are hidden info. **Policy: low–high
depending on source; rejected for lack of a clean data source.**

### R5. Lobby scouting / player look-ups
Champ select de-anonymising is forbidden [riot-general]. Loading-screen scouting
(as Porofessor/iTero do) is allowed but needs a Riot API production key, and it
scouts other players rather than coaching you. **Policy: high in champ select,
low at loading; rejected as off-mission.**

### R6. Augment win-rate numbers (including your personal ones)
Explicitly forbidden [riot-lol]. Use tiers, ranks and percentiles only.
**Policy: high.**

### R7. Importing runes/item sets into the client
Policy allows it (OP.GG, Porofessor do [opgg-status], [porofessor]), but it needs
LCU writes or writing item-set files, which breaks the app's read-only rule.
Revisit as an explicit opt-in later. **Policy: none; rejected by design.**

### R8. Enemy exact gold, MMR/rank estimators
Enemy gold is not in the client (v1's item-value estimate is fine); MMR
calculators are banned [riot-general]. **Policy: high.**

### R9. Overlay, sounds, TTS callouts
Against product decisions (second monitor, silent). **Rejected by design.**

### R10. Replay/VOD analysis
Riot is restricting replay access (26.20, delayed to 26.21) [sheep-replays], and it
is a large separate product. **Rejected for now.**

### R11. "Do it now" macro commands ("go Baron", "fight now")
"Apps that dictate player decisions" are unapproved [riot-lol]. Show facts (Q6)
and options, not orders. **Policy: high as commands, low as facts.**

### R12. Memory reading for richer data
Blocked by Vanguard for unknown apps from 6 Oct 2026 [riot-mem] and against the
app's principles. **Policy: high.**

---

## 7. Recommended roadmap (top 8, in order)

1. **Q4 Mode rule overrides**: fixes Mayhem advice that is wrong today; tiny. (S)
2. **Q1 Shopping mode**: the moment ARAM players actually look at the second monitor. (S)
3. **V1 Reroll advisor**: original, high value per Mayhem game, uses existing OCR. (M)
4. **V3 → V2 Augment detection + augment-aware items**: the app's best feature. (M+M)
5. **Q5 + Q6 + Q7 Threat-board enrichments**: the legal answer to "ult timers". (S)
6. **V10 Death review (+ Q9)**: the strongest self-coaching from data already recorded. (M)
7. **V5 Resist-aware pen / EHP split**: makes item reasons numeric. (M)
8. **V11 Trends and goals**: turns single recaps into improvement over weeks. (M)

Before any of these: make v1's augment UI show ranks only (no win rates) and
keep several options visible (section 1).

---

## Sources

[riot-lol]: https://developer.riotgames.com/docs/lol
[riot-lol-policy]: https://support-developer.riotgames.com/hc/en-us/articles/22698698001939-League-of-Legends
[riot-general]: https://developer.riotgames.com/policies/general
[riot-3p]: https://support.riotgames.com/hc/en-us/articles/225266848
[riot-mem]: https://support.riotgames.com/riot/performance/game-memory-access-removed-for-third-party-apps
[s20-2014]: https://www.surrenderat20.net/2014/05/red-post-collection-riots-stance-on-3rd.html
[porofessor]: https://www.borskollen.se/pressmeddelanden/porofessor-by-m-o-b-a-network-surpasses-15-million-downloads-solidifying-premier-status-among-league-of-legends-players
[blitz]: https://blitz.gg/lol/aram-mayhem-update
[moba-guide]: https://mobalytics.gg/lol/guides/mobalytics-beginners-guide
[moba-gpi]: https://mobalytics.gg/lol/gpi
[moba-mayhem]: https://mobalytics.gg/lol/champions/blitzcrank/mayhem-builds
[ugg]: https://www.tipranks.com/news/press-releases/enthusiast-gamings-u-gg-launches-valorant-desktop-app
[opgg-lol]: https://op.gg/desktop/en/games/lol
[opgg-status]: https://op.gg/desktop/it/status
[itero]: https://itero.gg/articles/overwolf
[itero-eg]: https://europeangaming.eu/portal/latest-news/2025/11/18/196546/the-app-you-need-to-download-if-youre-tired-of-throwing-in-league-of-legends/
[giantx]: https://giantx.gg/news/giantx-acquires-league-of-legends-ai-coaching-start-up-itero-gaming
[yourgg]: https://gengesports.notion.site/2026-01-08-Update-Arena-Mayhem-Augment-Tier-List-2e26548b309e80cd879ac6d4dfd200c2
[lola]: https://lolalytics.com/lol/ahri/build/
[wiki-mayhem]: https://wiki.leagueoflegends.com/en-us/ARAM:_Mayhem
[patch-26-3]: https://leagueoflegends.com/en-us/news/game-updates/patch-26-3-notes/
[mayhem-26-12]: https://arammayhem.com/patch/26-12/
[ow-gep]: https://dev.overwolf.com/ow-native/live-game-data-gep/supported-games/league-of-legends
[sheep-replays]: https://www.sheepesports.com/de/articles/riot-delays-league-of-legends-replay-restrictions/en

- Riot LoL docs and game policy (integrity rules, approved/unapproved use cases): [riot-lol], [riot-lol-policy]
- Riot general developer policies: [riot-general]
- Riot third-party stance and memory-access change: [riot-3p], [riot-mem]; 2014 timer ruling: [s20-2014]
- Competitors: [porofessor], [blitz], [moba-guide], [moba-gpi], [moba-mayhem], [ugg], [opgg-lol], [opgg-status], [itero], [itero-eg], [giantx], [yourgg], [lola]
- Mayhem mechanics and patches: [wiki-mayhem], [patch-26-3], [mayhem-26-12]; 26.21 augments: https://gameriv.com/lol-patch-26-21-brings-new-aram-mayhem-augments-get-jinxed-totentanz-and-more/
- Live API fields: [ow-gep]; replay restrictions: [sheep-replays]
