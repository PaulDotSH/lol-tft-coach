//! Lane calls on top of the advice (level races with my lane opponent, a
//! threat's item just finished) and quiet tips: CS pace, vision, gold held.
//! Facts from the scoreboard and my own gold; never positions or cooldowns.

use crate::tracker::Tracker;
use model::*;

const CONTROL_WARD: ItemId = 2055;
/// AP-only items below this are enchanter support items (Shurelya's 50, Redemption 30,
/// Moonstone 25), not a damage spike; mage items start around Liandry's 60.
const SPIKE_AP: f32 = 60.0;
const STEALTH_WARD: ItemId = 3340;
const LEVEL_SPIKES: [u32; 3] = [6, 11, 16];
/// A level race is called this long (s) after the level-up; later it is old news.
const LEVEL_CALL_S: f32 = 60.0;
/// CS behind the average pace worth a tip, from CS_FROM on.
const CS_BEHIND: f32 = 10.0;
const CS_FROM: f32 = 300.0;
/// Vision under this share of the average by now, from VISION_FROM on.
const VISION_LOW: f32 = 0.7;
const VISION_FROM: f32 = 600.0;
/// Control Ward reminders from this time on.
const WARD_FROM: f32 = 240.0;
/// Supports swap to Oracle Lens from here.
const ORACLE_FROM: f32 = 540.0;
/// Gold held this long (s) while Buy next is affordable is worth a back.
const HOLD_S: f32 = 60.0;
/// "Held" means gold stayed above this share of what I have now (no purchase).
const HOLD_SHARE: f32 = 0.8;
const HOLD_MIN_GOLD: f32 = 800.0;

fn clock(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn annotate(a: &mut GameAdvice, snap: &GameSnapshot, sd: &StaticData, tracker: &Tracker) {
    let Some(me) = snap.players.get(snap.me) else { return };
    let rift = snap.mode == Mode::Rift;
    let t = snap.time;
    let opp = a
        .threats
        .iter()
        .find(|t| t.lane_opp)
        .and_then(|th| snap.players.iter().enumerate().find(|(_, p)| p.champ == th.champ && p.team != me.team));
    let mut calls = Vec::new();

    // Level races with my lane opponent (junglers have none).
    if rift
        && !me.dead
        && me.position != Some(Lane::Jungle)
        && let Some((oi, o)) = opp
    {
        let name = sd.champ_name(o.champ);
        let fresh = |i, l| tracker.reached(i, l).is_some_and(|at| t - at <= LEVEL_CALL_S);
        if let Some(&l) = LEVEL_SPIKES.iter().rev().find(|&&l| o.level >= l && me.level < l)
            && fresh(oi, l)
        {
            calls.push(Call {
                key: format!("lvl-opp-{l}"),
                text: format!("{name} hit level {l} before you ({} vs {}): play safe until you do", o.level, me.level),
                say: format!("{name} hit {l} first. Play safe."),
            });
        } else if me.level >= 6 && o.level < 6 && fresh(snap.me, 6) {
            calls.push(Call {
                key: "lvl-me-6".into(),
                text: format!("You hit 6 before {name}: your all-in window until they do"),
                say: "You hit six first. Look for the all-in.".into(),
            });
        }
    }

    // Damage items my lane opponent or the top threat just finished (a Locket or a Shurelya's is no spike),
    // one call for items finished together.
    let damage = |id| {
        sd.items
            .get(&id)
            .is_some_and(|it| it.stats.ad + it.stats.attack_speed + it.stats.crit > 0.0 || it.stats.ap >= SPIKE_AP)
    };
    for (i, th) in a.threats.iter().enumerate() {
        if !(th.lane_opp || i == 0) {
            continue;
        }
        let ids: Vec<ItemId> = th.new_items.iter().copied().filter(|&id| damage(id)).collect();
        if ids.is_empty() {
            continue;
        }
        let names = ids.iter().map(|&id| sd.item_name(id)).collect::<Vec<_>>().join(" and ");
        let n = th.legendaries;
        calls.push(Call {
            key: format!("item-{}-{}", th.champ, ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join("-")),
            text: format!("{} finished {names}: {n} legendary item{} now, respect the spike", th.name, if n == 1 { "" } else { "s" }),
            say: format!("{} finished {names}.", th.name),
        });
    }

    let mut tips = Vec::new();
    let mut tip = |key: &str, kind: &str, text: String| tips.push(LiveTip { key: key.into(), kind: kind.into(), text });

    // CS pace against the champion average (supports don't farm).
    if rift
        && t >= CS_FROM
        && me.position != Some(Lane::Support)
        && !crate::jungle::is_jungler(me)
        && let (Some(diff), Some(avg)) = (a.me.cs_diff, a.me.cs_benchmark)
        && diff <= -CS_BEHIND
    {
        tip(
            "cs",
            "lane",
            format!("CS {:.0} behind the average pace ({:.1}/min vs {avg:.1}): last-hit every wave, catch side waves", -diff, a.me.cs_per_min),
        );
    }

    // Vision.
    if rift
        && t >= VISION_FROM
        && let Some(b) = a.me.vision_benchmark.filter(|b| *b > 0.0)
        && a.me.vision < VISION_LOW * b
    {
        tip("vision-low", "vision", format!("Vision score {:.0} vs about {b:.0} by now: ward the river and the next objective", a.me.vision));
    }
    if rift && t >= WARD_FROM && !me.items.contains(&CONTROL_WARD) {
        let text = if me.dead { "Buy a Control Ward while you're dead (75 g)" } else { "Next back: buy a Control Ward (75 g)" };
        tip("control-ward", "vision", text.into());
    }
    if rift && t >= ORACLE_FROM && me.position == Some(Lane::Support) && me.items.contains(&STEALTH_WARD) {
        tip("oracle", "vision", "Swap your trinket to Oracle Lens to clear their wards".into());
    }

    // Gold held with Buy next affordable, when no objective call already says back.
    if rift
        && !me.dead
        && let Some(item) = a.gold_eta.as_ref().filter(|e| e.missing == 0).and_then(|e| e.item)
        && !a.calls.iter().any(|c| c.key.starts_with("back-"))
    {
        let held = tracker.held_for((snap.my_gold * HOLD_SHARE).max(HOLD_MIN_GOLD), t);
        if held >= HOLD_S {
            tip(
                "back",
                "back",
                format!("You've held {:.0} gold for {}: back after you push the wave and buy {}", snap.my_gold, clock(held), sd.item_name(item)),
            );
        }
    }

    a.calls.extend(calls);
    a.tips = tips;
}
