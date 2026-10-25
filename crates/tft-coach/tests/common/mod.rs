//! Hand-built fixtures: a small set (8 units, 6 traits, components and a few finished items),
//! four MetaTFT-like comps, and snapshot / tracker helpers. Not the tft-data crate.
#![allow(dead_code)]

use std::collections::HashMap;
use tft_coach::*;
use tft_model::*;

// ---- traits ----
pub const DEFENDER: &str = "DA_18_Defender";
pub const DUELIST: &str = "DA_18_Duelist";
pub const SORCERER: &str = "DA_18_Sorcerer";
pub const COVEN: &str = "DA_18_Coven";
pub const BRUISER: &str = "DA_18_Bruiser";
/// Unique: only Taric has it.
pub const GREENFATHER: &str = "DA_18_Greenfather";

// ---- units ----
pub const GAREN: &str = "DA_18_Garen";
pub const LUX: &str = "DA_18_Lux";
pub const VI: &str = "DA_18_Vi";
pub const AHRI: &str = "DA_18_Ahri";
pub const LEONA: &str = "DA_18_Leona";
pub const RENGAR: &str = "DA_18_Rengar";
/// Same traits as Lux.
pub const MORGANA: &str = "DA_18_Morgana";
pub const TARIC: &str = "DA_18_Taric";

// ---- items ----
pub const SWORD: &str = "DA_BFSword";
pub const ROD: &str = "DA_NeedlesslyLargeRod";
pub const TEAR: &str = "DA_TearOfTheGoddess";
pub const BELT: &str = "DA_GiantsBelt";
pub const VEST: &str = "DA_ChainVest";
pub const CLOAK: &str = "DA_NegatronCloak";
pub const GLOVE: &str = "DA_SparringGloves";
pub const BOW: &str = "DA_RecurveBow";
pub const SPATULA: &str = "DA_Spatula";
pub const BT: &str = "DA_Bloodthirster";
pub const IE: &str = "DA_InfinityEdge";
pub const TITANS: &str = "DA_TitansResolve";
pub const RABADON: &str = "DA_RabadonsDeathcap";
pub const JG: &str = "DA_JeweledGauntlet";
pub const ARCHANGEL: &str = "DA_ArchangelsStaff";
pub const GUINSOO: &str = "DA_GuinsoosRageblade";
pub const WARMOGS: &str = "DA_WarmogsArmor";
pub const DEF_EMBLEM: &str = "DA_18_DefenderEmblem";
pub const RABADON_RADIANT: &str = "DA_RabadonsDeathcapRadiant";
pub const ARTIFACT: &str = "DA_Artifact_Deathfire";

// ---- augments ----
pub const AUG_TANK: &str = "DA_Aug_Tank";
pub const AUG_DUEL: &str = "DA_Aug_Duel";
pub const AUG_ECON: &str = "DA_Aug_Econ";

// ---- comps ----
pub const RENGAR_COMP: &str = "1";
pub const AHRI_COMP: &str = "2";
pub const DUEL_COMP: &str = "3";
pub const TARIC_COMP: &str = "4";

fn name(api: &str) -> String {
    api.trim_start_matches("DA_18_").trim_start_matches("DA_").to_string()
}

fn unit(api: &str, cost: u32, traits: &[&str]) -> (Api, TftUnit) {
    let u = TftUnit { api: api.into(), name: name(api), cost, traits: traits.iter().map(|t| t.to_string()).collect(), icon: String::new() };
    (api.into(), u)
}

fn tr(api: &str, breakpoints: &[u32]) -> (Api, TftTrait) {
    (api.into(), TftTrait { api: api.into(), name: name(api), breakpoints: breakpoints.to_vec(), icon: String::new() })
}

fn item(api: &str, display: &str, from: &[&str]) -> TftItem {
    TftItem {
        api: api.into(),
        name: display.into(),
        components: from.iter().map(|c| c.to_string()).collect(),
        component: from.is_empty(),
        emblem: false,
        radiant: false,
        artifact: false,
        icon: String::new(),
    }
}

fn aug(api: &str) -> (Api, TftAugment) {
    (api.into(), TftAugment { api: api.into(), name: name(api), tier: AugmentTier::Gold, desc: String::new(), icon: String::new() })
}

pub fn set() -> TftSetData {
    let units = [
        unit(GAREN, 1, &[DEFENDER, BRUISER]),
        unit(LUX, 1, &[SORCERER, COVEN]),
        unit(VI, 2, &[DUELIST, BRUISER]),
        unit(AHRI, 3, &[SORCERER, DUELIST]),
        unit(LEONA, 3, &[DEFENDER, COVEN]),
        unit(RENGAR, 4, &[DEFENDER, DUELIST]),
        unit(MORGANA, 4, &[COVEN, SORCERER]),
        unit(TARIC, 5, &[GREENFATHER, DEFENDER]),
    ];
    let traits = [
        tr(DEFENDER, &[2, 4]),
        tr(DUELIST, &[2, 4]),
        tr(SORCERER, &[2, 4]),
        tr(COVEN, &[2, 3]),
        tr(BRUISER, &[2, 4]),
        tr(GREENFATHER, &[1]),
    ];
    let mut items = vec![
        item(SWORD, "B.F. Sword", &[]),
        item(ROD, "Needlessly Large Rod", &[]),
        item(TEAR, "Tear of the Goddess", &[]),
        item(BELT, "Giant's Belt", &[]),
        item(VEST, "Chain Vest", &[]),
        item(CLOAK, "Negatron Cloak", &[]),
        item(GLOVE, "Sparring Gloves", &[]),
        item(BOW, "Recurve Bow", &[]),
        item(SPATULA, "Spatula", &[]),
        item(BT, "Bloodthirster", &[SWORD, CLOAK]),
        item(IE, "Infinity Edge", &[SWORD, GLOVE]),
        item(TITANS, "Titan's Resolve", &[BOW, VEST]),
        item(RABADON, "Rabadon's Deathcap", &[ROD, ROD]),
        item(JG, "Jeweled Gauntlet", &[ROD, GLOVE]),
        item(ARCHANGEL, "Archangel's Staff", &[ROD, TEAR]),
        item(GUINSOO, "Guinsoo's Rageblade", &[ROD, BOW]),
        item(WARMOGS, "Warmog's Armor", &[BELT, BELT]),
        TftItem { emblem: true, ..item(DEF_EMBLEM, "Defender Emblem", &[SPATULA, VEST]) },
        TftItem { radiant: true, component: false, ..item(RABADON_RADIANT, "Radiant Rabadon's Deathcap", &[]) },
        TftItem { artifact: true, component: false, ..item(ARTIFACT, "Deathfire Grasp", &[]) },
    ];
    TftSetData {
        version: "test".into(),
        set: 18,
        units: units.into_iter().collect(),
        traits: traits.into_iter().collect(),
        items: items.drain(..).map(|i| (i.api.clone(), i)).collect(),
        augments: [aug(AUG_TANK), aug(AUG_DUEL), aug(AUG_ECON)].into_iter().collect::<HashMap<_, _>>(),
    }
}

fn v(x: &[&str]) -> Vec<Api> {
    x.iter().map(|s| s.to_string()).collect()
}

fn build(unit: &str, items: &[&str]) -> CarryBuild {
    CarryBuild { unit: unit.into(), items: v(items), avg_place: 3.8, games: 500 }
}

#[allow(clippy::too_many_arguments)]
fn comp(
    id: &str,
    name: &str,
    units: &[&str],
    stars: &[&str],
    traits: &[(&str, u32)],
    avg: f32,
    games: u32,
    lvl: &str,
    builds: Vec<CarryBuild>,
    augments: &[&str],
) -> MetaComp {
    let items = builds.iter().flat_map(|b| b.items.clone()).collect();
    MetaComp {
        id: id.into(),
        name: name.into(),
        units: v(units),
        stars: v(stars),
        traits: traits.iter().map(|(t, n)| (t.to_string(), *n)).collect(),
        avg_place: avg,
        games,
        levelling: Some(lvl.into()),
        builds,
        items,
        augments: v(augments),
    }
}

pub fn meta() -> TftMeta {
    TftMeta {
        queue: QUEUE_RANKED,
        updated: 0,
        comps: vec![
            comp(RENGAR_COMP, "Defender Rengar", &[GAREN, LEONA, VI, RENGAR, TARIC], &[], &[(DEFENDER, 4), (DUELIST, 2)], 4.0, 5000, "lvl 8",
                vec![build(RENGAR, &[BT, IE, TITANS])], &[AUG_TANK, AUG_DUEL]),
            comp(AHRI_COMP, "Sorcerer Ahri", &[LUX, AHRI, MORGANA, LEONA], &[AHRI, LUX], &[(SORCERER, 4), (COVEN, 2)], 4.2, 4000, "lvl 7",
                vec![build(AHRI, &[RABADON, JG, ARCHANGEL]), build(MORGANA, &[GUINSOO])], &[AUG_ECON]),
            comp(DUEL_COMP, "Duelist Vi", &[VI, AHRI, RENGAR, GAREN], &[], &[(DUELIST, 4), (BRUISER, 2)], 4.4, 3000, "lvl 8",
                vec![build(VI, &[IE, BT])], &[]),
            comp(TARIC_COMP, "Greenfather Taric", &[TARIC, LEONA, MORGANA, LUX], &[], &[(GREENFATHER, 1), (COVEN, 3)], 4.6, 800, "lvl 9",
                vec![build(TARIC, &[WARMOGS])], &[]),
        ],
        units: vec![],
        items: vec![],
        traits: vec![],
        augments: vec![],
    }
}

pub fn shop(slots: &[Option<&str>]) -> Vec<Option<Api>> {
    slots.iter().map(|s| s.map(|u| u.to_string())).collect()
}

pub fn traits(t: &[(&str, u32)]) -> Vec<(Api, u32)> {
    t.iter().map(|(a, n)| (a.to_string(), *n)).collect()
}

pub fn items(i: &[&str]) -> Vec<Api> {
    v(i)
}

/// My board at a stage with some gold, an empty shop, everything else unread.
pub fn snap(stage: (u32, u32), gold: u32) -> TftSnapshot {
    TftSnapshot { stage: Some(stage), gold: Some(gold), shop: shop(&[None; 5]), ..Default::default() }
}

/// Buys each unit in turn (it shows in slot 0, then the slot empties as gold drops by its cost).
pub fn own(t: &mut Tracker, s: &TftSetData, stage: (u32, u32), units: &[&str]) {
    for u in units {
        let cost = s.units[*u].cost;
        t.observe(&TftSnapshot { shop: shop(&[Some(u), None, None, None, None]), ..snap(stage, 50) }, s);
        t.observe(&TftSnapshot { shop: shop(&[None; 5]), ..snap(stage, 50 - cost) }, s);
    }
}

pub fn advise_with(snap: &TftSnapshot, t: &Tracker, s: &TftSetData, m: &TftMeta, keep: Option<&str>) -> TftAdvice {
    advise(&CoachInput { snap, tracker: t, set: s, meta: m, hooks: &StandardHooks, keep })
}

/// Observes the snapshot, then advises on it (no last target).
pub fn tick(t: &mut Tracker, s: &TftSetData, m: &TftMeta, snap: &TftSnapshot) -> TftAdvice {
    t.observe(snap, s);
    advise_with(snap, t, s, m, None)
}

pub fn ids(picks: &[CompPick]) -> Vec<&str> {
    picks.iter().map(|p| p.comp.as_str()).collect()
}

pub fn keys(a: &TftAdvice) -> Vec<&str> {
    a.calls.iter().map(|c| c.key.as_str()).collect()
}
