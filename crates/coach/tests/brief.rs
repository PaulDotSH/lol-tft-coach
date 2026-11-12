//! The coach's note: the fact sheet of a recorded flex game, the rules that
//! connect its facts, and the check every line has to pass.

use coach::brief;
use model::*;
use std::collections::HashMap;

fn read(dir: &str, name: &str) -> String {
    std::fs::read_to_string(format!("{}/../../fixtures/{dir}/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap().replace("\r\n", "\n")
}

fn sd() -> StaticData {
    StaticData {
        version: "16.21.1".into(),
        items: data::ddragon::parse_items(&read("data", "ddragon-item.json")).unwrap(),
        champs: data::ddragon::parse_champions(&read("data", "ddragon-champion.json")).unwrap(),
        item_groups: HashMap::new(),
        runes: HashMap::new(),
    }
}

fn analyze(game: &str, timeline: &str, puuid: &str, sd: &StaticData) -> MatchAnalysis {
    let json = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
    let d = clients::history::parse_match(&json(game), Some(puuid), Some(&json(timeline))).unwrap();
    coach::analyze_match(&d, sd, None, None, coach::winprob::Model::shipped())
}

fn flex(sd: &StaticData) -> MatchAnalysis {
    analyze(&read("clients", "match_game_flex.json"), &read("clients", "match_timeline_flex.json"), "me-puuid", sd)
}

fn no_augments() -> HashMap<AugmentId, String> {
    HashMap::new()
}

fn session(a: &MatchAnalysis) -> Session {
    let repeat = |text: &str| SessionRepeat { text: text.into(), games: 3, of: 4 };
    Session {
        start: 0,
        end: 0,
        game_ids: vec![a.game.game_id],
        wins: 1,
        losses: 3,
        repeats: vec![repeat("Died before 10:00"), repeat("No Control Wards bought"), repeat("Behind your lane opponent at 15:00"), repeat("Died 6 or more times")],
        goals: vec![],
    }
}

fn lines(n: &GameNote) -> Vec<&str> {
    n.takeaways.iter().chain(&n.focus).map(|l| l.text.as_str()).collect()
}

fn all_pass(a: &MatchAnalysis, s: Option<&Session>, sd: &StaticData, augments: &HashMap<AugmentId, String>) -> GameNote {
    let facts = brief::facts(a, s, sd, augments);
    let names = brief::names(sd, augments);
    let n = brief::compose(a, s, sd, augments);
    for l in n.takeaways.iter().chain(&n.focus) {
        assert!(brief::passes(l, &facts, &names), "fails the check: {l:?}\n{facts:#?}");
    }
    n
}

#[test]
fn flex_fact_sheet() {
    let sd = sd();
    let a = flex(&sd);
    let sheet: String = brief::facts(&a, Some(&session(&a)), &sd, &no_augments()).iter().map(|f| format!("{}. [{:?}] {}\n", f.id, f.kind, f.text)).collect();
    let path = format!("{}/../../fixtures/coach/brief_flex_facts.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(&path, &sheet).unwrap();
    }
    assert_eq!(sheet, read("coach", "brief_flex_facts.txt"));
}

#[test]
fn flex_note_connects_facts() {
    let sd = sd();
    let a = flex(&sd);
    let n = brief::note(&a, None, &sd, &no_augments()).unwrap();
    assert_eq!(
        lines(&n),
        [
            "Vision fell short: 25 wards placed against 43 for Braum, and 0 Control Wards bought.",
            "You won your lane (+461 gold at 14:00, ahead from 8:00) and kept it going in the fights at 21:38 in the enemy jungle, 23:18 in the river and 26:49 in the enemy jungle.",
            "Kill participation 65%.",
            "At most 1 death before 14:00. Two early deaths came outside fights your team won: 4:21 in the river to Aphelios and 11:00 in bot lane to Braum.",
        ]
    );
    let goods: Vec<Option<bool>> = n.takeaways.iter().map(|l| l.good).collect();
    assert_eq!(goods, [Some(false), Some(true), Some(true)]);
    let cited: Vec<u32> = n.facts.iter().map(|f| f.id).collect();
    assert_eq!(cited, [3, 8, 11, 13, 14, 15, 30, 31, 33]);
    assert_eq!(n.rules_v, coach::consts::NOTE_RULES_V);
}

#[test]
fn a_session_habit_meets_this_games_instance() {
    let sd = sd();
    let a = flex(&sd);
    let n = brief::note(&a, Some(&session(&a)), &sd, &no_augments()).unwrap();
    assert_eq!(
        lines(&n),
        [
            "No Control Wards again (3 of 4 games this session, this one included), and 25 wards placed against 43 for Braum.",
            "You won your lane (+461 gold at 14:00, ahead from 8:00) and kept it going in the fights at 21:38 in the enemy jungle, 23:18 in the river and 26:49 in the enemy jungle.",
            "Kill participation 65%.",
            "At most 1 death before 14:00. Dying before 10:00 keeps coming back (3 of 4 games this session); this game at 4:21 in the river to Aphelios.",
        ]
    );
    let mut other = session(&a);
    other.game_ids = vec![1];
    assert_eq!(brief::note(&a, Some(&other), &sd, &no_augments()), brief::note(&a, None, &sd, &no_augments()));
}

#[test]
fn a_lead_lost_names_the_fights_and_held_gold_names_buy_next() {
    let sd = sd();
    let mut a = flex(&sd);
    a.game.win = false;
    a.gold.iter_mut().for_each(|p| p.team = p.team.min(1000));
    let at = a.gold.iter().position(|p| p.time >= 1200.0).unwrap();
    a.gold[at].team = 4200;
    a.fights.iter_mut().filter(|f| f.start >= 1200.0).for_each(|f| (f.won, f.lost) = (f.lost, f.won));
    a.review.deaths[1].unspent = 1400;
    a.review.deaths[1].buy_next = Some(3068);
    a.review.deaths[1].could_buy = true;
    a.review.deaths[2].unspent = 1250;
    let n = all_pass(&a, None, &sd, &no_augments());
    let l = lines(&n);
    assert!(l.contains(&"Your team was 4,200 gold ahead at 20:00 and lost; the fights after it went their way, at 21:38 in the enemy jungle, 23:18 in the river and 26:49 in the enemy jungle."), "{l:#?}");
    assert!(l.contains(&"You died holding gold: 1,400 at 11:00 and 1,250 at 23:18; Buy next Sunfire Aegis was already affordable at 11:00."), "{l:#?}");
    assert!(!l.iter().any(|t| t.starts_with("You won your lane")), "one line for the lead: {l:#?}");
    assert_eq!(n.takeaways.iter().filter(|t| t.good == Some(true)).count(), 1);
}

#[test]
fn composer_lines_always_pass_the_check() {
    let sd = sd();
    let base = flex(&sd);
    let mut variants = vec![base.clone()];
    let mut lost = base.clone();
    lost.game.win = false;
    lost.fights.iter_mut().for_each(|f| (f.won, f.lost, f.died_first) = (f.lost, f.won, true));
    lost.gold.iter_mut().for_each(|p| p.team = -p.team);
    lost.marks.iter_mut().for_each(|m| (m.ours, m.presence) = (!m.ours, Some(Presence::Away)));
    lost.review.deaths.iter_mut().for_each(|d| (d.unspent, d.traded, d.buy_next, d.could_buy, d.spikes) = (2100, false, Some(6653), true, vec![(201, 6)]));
    lost.rich_marks = vec![12, 18, 21];
    lost.phases.iter_mut().for_each(|p| p.gold_lead = Some(-900));
    lost.game.deaths = 8;
    variants.push(lost);
    let mut comeback = base.clone();
    comeback.gold.iter_mut().for_each(|p| p.team -= 9000);
    variants.push(comeback);
    for f in [
        Focus { text: "Reach 5.0 CS/min by 10:00".into(), why: "You had 1.0 CS/min.".into(), goal: Some(Goal { kind: GoalKind::Cs10, target: 5.0 }) },
        Focus { text: "Play the lane safer against Braum: farm until your first item".into(), why: "You were 900 gold behind at 15:00.".into(), goal: None },
        Focus { text: "Vision 2.6/min: a Control Ward every back".into(), why: "0 Control Wards bought.".into(), goal: Some(Goal { kind: GoalKind::Vision, target: 2.6 }) },
        Focus { text: "Back and spend before you sit on 1000+ gold".into(), why: "You died 3× holding 1000+ gold.".into(), goal: Some(Goal { kind: GoalKind::Unspent, target: 1000.0 }) },
    ] {
        for v in variants.clone() {
            variants.push(MatchAnalysis { focus: Some(f.clone()), ..v });
        }
    }
    let mut mayhem = base.clone();
    mayhem.game.mode = Mode::Mayhem;
    mayhem.game.augments = vec![1, 2];
    mayhem.takeaways.push(Takeaway { good: true, text: "Damage 900/min vs 700 average on Nautilus".into() });
    let augments: HashMap<AugmentId, String> = [(1, "Goliath".to_string()), (2, "Tank Engine".to_string())].into();
    let n = all_pass(&mayhem, None, &sd, &augments);
    assert!(lines(&n).contains(&"Damage 900/min vs 700 average on Nautilus with Goliath and Tank Engine."), "{:#?}", lines(&n));

    let mut connected = 0;
    for v in &variants {
        for s in [None, Some(session(v))] {
            let n = all_pass(v, s.as_ref(), &sd, &no_augments());
            assert!(n.takeaways.len() <= coach::consts::NOTE_TAKEAWAYS);
            connected += n.takeaways.iter().chain(&n.focus).filter(|l| l.facts.len() >= 2).count();
        }
    }
    assert!(connected >= variants.len());
}

#[test]
fn no_note_for_a_remake_or_a_quiet_game() {
    let sd = sd();
    let mut a = flex(&sd);
    a.takeaways.truncate(1);
    a.focus = None;
    a.review.deaths.clear();
    a.game.win = false;
    assert_eq!(brief::note(&a, None, &sd, &no_augments()), None);
    a.game.remake = true;
    a.takeaways.clear();
    assert_eq!(brief::note(&a, None, &sd, &no_augments()), None);
}

fn fact(id: u32, text: &str) -> Fact {
    Fact { id, kind: FactKind::Deaths, good: None, text: text.into() }
}

fn line(text: &str, facts: &[u32]) -> NoteLine {
    NoteLine { text: text.into(), facts: facts.to_vec(), good: Some(false) }
}

#[test]
fn check_keeps_only_lines_built_from_their_facts() {
    let facts = [
        fact(1, "Death at 4:21 in the river to Aphelios, holding 1,250 gold; Buy next Sunfire Aegis was affordable."),
        fact(2, "Ashe and Nautilus vs Aphelios and Braum: Won lane: +461 gold at 14:00."),
        fact(3, "Fight at 23:18 in the river to Viego: kills 4–1."),
    ];
    let names: Vec<String> = ["Aphelios", "Braum", "Ashe", "Nautilus", "Viego", "Vi", "Kindred", "Sunfire Aegis"].map(String::from).to_vec();
    let ok = |text: &str, ids: &[u32]| brief::passes(&line(text, ids), &facts, &names);
    assert!(ok("You died at 4:21 to Aphelios holding 1250 gold, when Sunfire Aegis was affordable.", &[1]));
    assert!(ok("Up +461 at 14:00 against Braum, then Viego won the fight at 23:18.", &[2, 3]));
    assert!(!ok("You died at 4:25 to Aphelios.", &[1]), "invented number");
    assert!(!ok("You died at 4:21 to Braum.", &[1]), "champion only in a fact it does not cite");
    assert!(!ok("You died at 4:21 to Kindred.", &[1]), "champion in no fact");
    assert!(!ok("Your lane: +461 gold, Sunfire Aegis.", &[2]), "item not in the cited facts");
    assert!(!ok("You died at 4:21.", &[9]), "unknown fact");
    assert!(!ok("You died at 4:21.", &[1, 9]), "one unknown fact");
    assert!(!ok("You died early.", &[]), "no citation");
    assert!(!ok(&"a".repeat(coach::consts::NOTE_MAX_CHARS), &[1]), "too long");

    let note = |takeaways: Vec<NoteLine>, focus: Option<NoteLine>| GameNote { takeaways, focus, facts: vec![], rules_v: 1 };
    let good = line("You died at 4:21 to Aphelios.", &[1]);
    let also = line("Won lane: +461 gold at 14:00.", &[2]);
    let bad = line("You died 5 times.", &[1]);
    let n = brief::check(&note(vec![good.clone(), bad.clone(), also.clone()], Some(bad.clone())), &facts, &names).unwrap();
    assert_eq!((n.takeaways, n.focus), (vec![good.clone(), also], None));
    assert_eq!(n.facts.iter().map(|f| f.id).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(brief::check(&note(vec![good, bad], None), &facts, &names), None);
}

#[test]
fn mock_review_note_is_the_composers() {
    let sd = sd();
    let path = format!("{}/../../web/src/mock/analysis.json", env!("CARGO_MANIFEST_DIR"));
    let a: MatchAnalysis = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let n = brief::note(&MatchAnalysis { note: None, ..a.clone() }, None, &sd, &no_augments());
    assert!(n.is_some());
    assert_eq!(a.note, n, "{}", serde_json::to_string(&n).unwrap());
}

#[test]
#[ignore]
fn real_games_on_disk() {
    let dir = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("lol-coach").join("debug");
    let sd = sd();
    let mut seen = Vec::new();
    for run in std::fs::read_dir(dir).unwrap().flatten() {
        let files: Vec<std::path::PathBuf> = std::fs::read_dir(run.path()).unwrap().flatten().map(|e| e.path()).collect();
        let find = |key: &str| files.iter().find(|p| p.to_string_lossy().contains(key)).map(|p| std::fs::read_to_string(p).unwrap());
        let (Some(game), Some(timeline), Some(me)) = (find("match-history_v1_games_"), find("game-timelines_"), find("current-summoner.json")) else { continue };
        let puuid = serde_json::from_str::<serde_json::Value>(&me).unwrap()["puuid"].as_str().unwrap().to_string();
        let a = analyze(&game, &timeline, &puuid, &sd);
        if seen.contains(&a.game.game_id) {
            continue;
        }
        seen.push(a.game.game_id);
        let n = all_pass(&a, None, &sd, &no_augments());
        println!("{} {} {}: {:#?}", a.game.game_id, sd.champ_name(a.game.champ), if a.game.win { "win" } else { "loss" }, lines(&n));
    }
    assert!(!seen.is_empty());
}
