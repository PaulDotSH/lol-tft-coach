//! Pair tips: lines from both kits and from the matchup page's numbers, on real notes and a
//! saved matchup page.

use coach::{CardData, matchup_numbers, pair_tips};
use model::*;
use std::collections::HashMap;

const MALPHITE: ChampKey = 54;
const SETT: ChampKey = 875;
const TEEMO: ChampKey = 17;
const GAREN: ChampKey = 86;
const MORDEKAISER: ChampKey = 82;
const MORGANA: ChampKey = 25;
const NAUTILUS: ChampKey = 111;

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn sd() -> StaticData {
    let mut champs = data::ddragon::parse_champions(&read("ddragon-champion.json")).unwrap();
    // The saved champion list carries no stats.
    for (key, range) in [(MALPHITE, 125.0), (SETT, 125.0), (TEEMO, 500.0), (GAREN, 175.0), (MORDEKAISER, 175.0), (MORGANA, 450.0), (NAUTILUS, 175.0)] {
        champs.get_mut(&key).unwrap().base.attack_range = range;
    }
    StaticData {
        version: "16.22.1".into(),
        items: data::ddragon::parse_items(&read("ddragon-item.json")).unwrap(),
        champs,
        item_groups: HashMap::new(),
        runes: HashMap::new(),
    }
}

fn kit(sd: &StaticData, notes: &LaneNotes, me: ChampKey, them: ChampKey) -> Vec<String> {
    pair_tips::kit(sd, notes, me, them)
}

#[test]
fn malphite_into_sett_talks_about_both_kits() {
    let (sd, notes) = (sd(), data::notes::lane_notes().unwrap());
    assert_eq!(
        kit(&sd, &notes, MALPHITE, SETT),
        [
            "Your E cuts attack speed and Sett fights with autos: use it when they start hitting you",
            "Sett's W shield soaks an all-in: go in with R once it is down",
            "Sett deals physical damage, which the armour from your W blunts, but Sett's W is true damage",
            "Sett is melee: poke with Q when they walk up to last-hit",
        ]
    );
    // Another champion into Sett reads differently, and so does Malphite into someone else.
    let teemo = kit(&sd, &notes, TEEMO, SETT);
    assert_eq!(teemo[0], "Your Q stops autos and Sett lives on them: take the fight while it is up");
    assert!(teemo.iter().any(|l| l.starts_with("You out-range Sett")), "{teemo:?}");
    assert!(teemo.iter().all(|l| !l.contains("armour")), "{teemo:?}");
    let garen = kit(&sd, &notes, MALPHITE, GAREN);
    assert!(garen.contains(&"Garen's passive heals poke back: use Q to set up a kill, not to chip".to_string()), "{garen:?}");
    let morde = kit(&sd, &notes, MALPHITE, MORDEKAISER);
    assert!(morde.contains(&"Mordekaiser deals magic damage: the armour from your W does little in this lane".to_string()), "{morde:?}");
    // Spell shield against a hook and a targeted ult: said once.
    let morgana = kit(&sd, &notes, MORGANA, NAUTILUS);
    assert_eq!(morgana.iter().filter(|l| l.starts_with("Your E blocks Nautilus's")).count(), 1, "{morgana:?}");
    assert_eq!(morgana[0], "Your E blocks Nautilus's Q: hold it for that");
    assert!(kit(&sd, &notes, MALPHITE, SETT).len() <= 4 && kit(&sd, &notes, SETT, MALPHITE) != kit(&sd, &notes, MALPHITE, SETT));
}

#[test]
fn no_kit_tags_no_lines_and_rule_tags_exist() {
    let sd = sd();
    assert!(kit(&sd, &LaneNotes::default(), 0, SETT).is_empty(), "unknown champion");
    let bare = LaneNotes::default();
    let lines = kit(&sd, &bare, MALPHITE, SETT);
    assert!(lines.iter().all(|l| !l.contains("Your E") && !l.contains("W shield")), "only what Data Dragon and traits say: {lines:?}");
    for tag in pair_tips::rule_tags() {
        assert!(data::notes::KIT_TAGS.contains(&tag) || pair_tips::DERIVED.contains(&tag), "rule tag {tag} is in no vocabulary");
    }
    for tag in data::notes::KIT_TAGS {
        assert!(pair_tips::rule_tags().contains(&tag), "kit tag {tag} is used by no rule");
    }
}

#[test]
fn matchup_page_numbers_for_malphite_into_sett() {
    let sd = sd();
    let page = data::lolalytics::parse_matchup_page(&read("lolalytics-vs-malphite-sett.html"), SETT).unwrap();
    assert_eq!(
        pair_tips::numbers(&sd, &page),
        [
            "55.1% into Sett against 53.4% across all matchups",
            "First item: Sunfire Aegis 55.2% (52% take it), Malignance 56.8% (21%), Frozen Heart 56.1% (13%), Thornmail 51.0% (6%)",
            "Boots: Plated Steelcaps 55.9% (78% take them), Mercury's Treads 48.3% (10%), Sorcerer's Shoes 61.2% (7%)",
            "Max order: QEW 55.7% (2.5k games), WQE 59.8% (261 games)",
            "Games over before 25:00: 57.7%; 25:00 to 35:00: 55.2%; later: 50.9%",
        ]
    );

    // Nothing to say: same win rate as usual, one row per table, one max order, flat by length.
    let flat = MatchupBuild {
        usual_wr: Some(page.win_rate + 0.4),
        slots: vec![page.slots[0][..1].to_vec()],
        boots: vec![],
        skill_max: page.skill_max[..1].to_vec(),
        by_length: page.by_length.iter().map(|l| LengthWin { wins: l.games / 2, ..l.clone() }).collect(),
        ..page.clone()
    };
    assert!(pair_tips::numbers(&sd, &flat).is_empty(), "{:?}", pair_tips::numbers(&sd, &flat));
    // A second max order from few games, or barely better, is left out.
    let thin = MatchupBuild { skill_max: vec![page.skill_max[0].clone(), SkillMax { games: 60, ..page.skill_max[1].clone() }], ..flat.clone() };
    assert!(pair_tips::numbers(&sd, &thin).is_empty());
}

#[test]
fn cards_carry_kit_lines_and_numbers_only_for_the_page_s_opponent() {
    let (sd, notes) = (sd(), data::notes::lane_notes().unwrap());
    let d = CardData { sd: &sd, notes: &notes, me: MALPHITE, keystone: None, spells: &HashMap::new(), memory: &[] };
    let mut cards = vec![coach::matchup_card(&d, SETT, None, None), coach::matchup_card(&d, GAREN, None, None)];
    assert!(cards[0].kit.is_empty() && cards[0].pair_more.len() > 2, "a hand-written list stands in for the kit lines");
    assert!(cards[1].pair.is_some() && cards[1].pair_more.is_empty() && !cards[1].kit.is_empty(), "one hand-written line: kit lines under it");
    assert!(cards[0].pair.as_deref().is_some_and(|p| p.contains("W")), "hand-written Malphite into Sett line");
    assert!(cards[0].pair_more.iter().any(|t| t.starts_with("Grasp of the Undying: ")), "{:?}", cards[0].pair_more);
    assert!(cards.iter().all(|c| c.numbers.is_empty()));

    let page = data::lolalytics::parse_matchup_page(&read("lolalytics-vs-malphite-sett.html"), SETT).unwrap();
    matchup_numbers(&mut cards, &sd, Some(&page));
    assert_eq!((cards[0].numbers.len(), cards[1].numbers.len()), (5, 0));
    // Build path: Doran's Ring and two potions, then one item per slot; the best build is another one.
    let path = &cards[0].path;
    assert_eq!((path[0].items.as_slice(), path[0].win_rate, path[0].games), ([1056, 2003, 2003].as_slice(), 56.14, 3048));
    assert_eq!(path[1..4].iter().map(|s| (s.items[0], s.games)).collect::<Vec<_>>(), [(3068, 2198), (3047, 3296), (3075, 1232)]);
    assert!((path[1].win_rate - 55.23).abs() < 0.01, "{}", path[1].win_rate);
    assert_eq!(cards[0].best_path.iter().map(|s| s.items[0]).take(3).collect::<Vec<_>>(), [3118, 3020, 3068]);
    assert!(cards[1].path.is_empty() && cards[1].best_path.is_empty());
    let same = MatchupBuild { best_path: page.path.clone(), ..page.clone() };
    assert!(pair_tips::paths(&same).1.is_empty(), "the best build is the most played one");
    let few = MatchupBuild { games: 60, ..page };
    let mut cards = vec![coach::matchup_card(&d, SETT, None, None)];
    matchup_numbers(&mut cards, &sd, Some(&few));
    matchup_numbers(&mut cards, &sd, None);
    assert!(cards[0].numbers.is_empty(), "a page from few games says nothing");

    // Tips the model wrote: under the keystone's name, mine only once it is known, and the kit lines step aside.
    let written = [
        PairTip { text: "Poke with your Q at max range.".into(), keystone: None },
        PairTip { text: "Q, then step back.".into(), keystone: Some("Arcane Comet".into()) },
        PairTip { text: "Auto when Grasp is charged.".into(), keystone: Some("Grasp of the Undying".into()) },
    ];
    let mut sd_runes = sd.clone();
    sd_runes.runes.insert(8437, RuneInfo { name: "Grasp of the Undying".into(), icon: String::new() });
    let mut cards = vec![coach::matchup_card(&d, GAREN, None, None)];
    assert!(!cards[0].kit.is_empty() && cards[0].ai.is_empty());
    coach::matchup_ai(&mut cards, &sd_runes, GAREN, &written, None);
    assert_eq!(cards[0].ai, ["Poke with your Q at max range.", "Arcane Comet: Q, then step back.", "Grasp of the Undying: Auto when Grasp is charged."]);
    assert!(cards[0].kit.is_empty() && cards[0].pair.is_some());
    coach::matchup_ai(&mut cards, &sd_runes, GAREN, &written, Some(8437));
    assert_eq!(cards[0].ai, ["Poke with your Q at max range.", "Grasp of the Undying: Auto when Grasp is charged."]);
    coach::matchup_ai(&mut cards, &sd_runes, SETT, &written, None);
    assert_eq!(cards.len(), 1, "no card for that opponent");
    // Asked in ranked solo/duo, flex, Clash and custom games only.
    assert_eq!([420, 440, 700, 0, -1].map(coach::ai_queue), [true; 5]);
    assert_eq!([400, 480, 490, 450, 2400].map(coach::ai_queue), [false; 5], "draft, Swiftplay, Quickplay, ARAM, Mayhem");

    // No tags, no hand-written line and no page: the champion note alone, as before.
    let bare = LaneNotes { champs: notes.champs.clone(), ..Default::default() };
    let d = CardData { notes: &bare, ..d };
    let c = coach::matchup_card(&d, GAREN, None, None);
    assert!(c.pair.is_none() && c.numbers.is_empty() && c.note.is_some());
}
