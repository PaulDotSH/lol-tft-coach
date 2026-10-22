//! Curated Mayhem augment tags (`augments.toml`) and what they do to item scoring.

use model::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Score shift (win-rate units) for an item an augment builds on / already covers.
pub const AUG_ITEM_BONUS: f32 = 0.025;
/// Items a picked quest augment asks for.
pub const AUG_QUEST_BONUS: f32 = 0.08;
/// Tenacity need kept when an augment grants tenacity.
pub const AUG_TENACITY_NEED: f32 = 0.3;
/// Tank item: this much health or armor + MR.
const TANK_ITEM_HP: f32 = 300.0;
const TANK_ITEM_RESISTS: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    OnHit,
    Crit,
    AttackSpeed,
    Haste,
    Ap,
    HealShield,
    Tank,
    Mana,
    Range,
    Tenacity,
    MoveSpeed,
    QuestItem,
}

impl Tag {
    pub fn label(self) -> &'static str {
        match self {
            Tag::OnHit => "on-hit",
            Tag::Crit => "crit",
            Tag::AttackSpeed => "attack speed",
            Tag::Haste => "ability haste",
            Tag::Ap => "AP",
            Tag::HealShield => "heal/shield power",
            Tag::Tank => "tank",
            Tag::Mana => "mana",
            Tag::Range => "attacks",
            Tag::Tenacity => "tenacity",
            Tag::MoveSpeed => "move speed",
            Tag::QuestItem => "quest",
        }
    }

    /// Item direction: +1 raises matching items, −1 lowers them (the augment already gives it).
    fn sign(self) -> f32 {
        match self {
            Tag::Haste => -1.0,
            Tag::Tenacity | Tag::MoveSpeed | Tag::QuestItem => 0.0,
            _ => 1.0,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Aug {
    pub id: AugmentId,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// Stat key → value; see the header of `augments.toml`.
    #[serde(default)]
    pub stats: HashMap<String, f32>,
    pub grants: Option<ItemId>,
    #[serde(default)]
    pub requires: Vec<ItemId>,
    #[serde(default)]
    pub keep_slot: bool,
}

#[derive(Deserialize)]
struct File {
    heal_shield_items: Vec<ItemId>,
    augment: Vec<Aug>,
}

static DATA: LazyLock<(File, HashMap<AugmentId, usize>)> = LazyLock::new(|| {
    let f: File = toml::from_str(include_str!("../augments.toml")).expect("augments.toml is valid");
    let idx = f.augment.iter().enumerate().map(|(i, a)| (a.id, i)).collect();
    (f, idx)
});

pub fn get(id: AugmentId) -> Option<&'static Aug> {
    DATA.1.get(&id).map(|&i| &DATA.0.augment[i])
}

/// Does `it` deliver what the tag is about?
pub fn item_has(tag: Tag, it: &Item) -> bool {
    let s = &it.stats;
    match tag {
        Tag::OnHit => it.tags.iter().any(|t| t == "OnHit"),
        Tag::Crit => s.crit > 0.0,
        Tag::AttackSpeed => s.attack_speed > 0.0,
        Tag::Haste => s.ability_haste > 0.0,
        Tag::Ap => s.ap > 0.0,
        Tag::HealShield => DATA.0.heal_shield_items.contains(&it.id),
        Tag::Tank => s.hp >= TANK_ITEM_HP || s.armor + s.mr >= TANK_ITEM_RESISTS,
        Tag::Mana => s.mana > 0.0,
        Tag::Range => s.attack_speed > 0.0 || item_has(Tag::OnHit, it),
        Tag::Tenacity => it.answers.tenacity > 0.0,
        Tag::MoveSpeed => it.boots || s.move_speed > 0.0,
        Tag::QuestItem => false,
    }
}

/// One picked augment (or the common ground of an unresolved pick) as scoring sees it.
pub struct Source {
    /// "Warlock Juicebox", or "your on-hit augment" when the pick is unresolved.
    pub name: String,
    pub tags: Vec<Tag>,
    pub requires: &'static [ItemId],
    pub keep_slot: bool,
}

#[derive(Default)]
pub struct AugEffects {
    pub sources: Vec<Source>,
}

impl AugEffects {
    /// Resolved picks use their own tags; unresolved ones the tags all candidates share.
    pub fn new(picks: &[MyAugment]) -> AugEffects {
        let sources = picks
            .iter()
            .filter_map(|p| match p.id {
                Some(id) => {
                    let a = get(id)?;
                    Some(Source { name: a.name.clone(), tags: a.tags.clone(), requires: &a.requires, keep_slot: a.keep_slot })
                }
                None => {
                    let cands: Vec<&Aug> = p.candidates.iter().map(|&c| get(c)).collect::<Option<_>>()?;
                    let first = cands.first()?;
                    let tags: Vec<Tag> =
                        first.tags.iter().copied().filter(|t| cands.iter().all(|a| a.tags.contains(t))).collect();
                    let t = tags.first()?;
                    Some(Source { name: format!("your {} augment", t.label()), tags, requires: &[], keep_slot: false })
                }
            })
            .collect();
        AugEffects { sources }
    }

    pub fn find(&self, tag: Tag) -> Option<&Source> {
        self.sources.iter().find(|s| s.tags.contains(&tag))
    }

    /// Item slots to leave free for quest rewards.
    pub fn keep_slots(&self) -> usize {
        self.sources.iter().filter(|s| s.keep_slot).count()
    }

    /// Score shift for `it` (win-rate units) and the source that moved it most.
    pub fn item_bonus(&self, it: &Item) -> (f32, Option<usize>) {
        let (mut total, mut top, mut src) = (0.0, 0.0, None);
        for (i, s) in self.sources.iter().enumerate() {
            let v = if s.requires.contains(&it.id) {
                AUG_QUEST_BONUS
            } else {
                // Strongest tag per augment, not the sum: one augment, one push.
                s.tags.iter().filter(|&&t| item_has(t, it)).map(|t| t.sign() * AUG_ITEM_BONUS).fold(0.0, |a: f32, b| {
                    if b.abs() > a.abs() { b } else { a }
                })
            };
            total += v;
            if v.abs() > top {
                (top, src) = (v.abs(), Some(i));
            }
        }
        (total, src)
    }
}

/// Owned item an offered augment builds on, e.g. ("Nashor's Tooth", "on-hit").
pub fn matches_owned<'a>(aug: AugmentId, sd: &'a StaticData, owned: &[ItemId]) -> Option<(&'a str, &'static str)> {
    let a = get(aug)?;
    let items = || owned.iter().filter_map(|i| sd.items.get(i)).filter(|i| i.completed);
    if let Some(it) = items().find(|i| a.requires.contains(&i.id)) {
        return Some((&it.name, "quest"));
    }
    a.tags.iter().filter(|t| t.sign() > 0.0).find_map(|&t| items().find(|i| item_has(t, i)).map(|i| (i.name.as_str(), t.label())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT_KEYS: [&str; 17] = [
        "ap", "ah", "ad", "armor", "mr", "hp", "range", "crit", "omnivamp", "life_steal", "hsp", "ms", "as_pct", "ad_pct",
        "hp_pct", "armor_pen_pct", "magic_pen_pct",
    ];

    #[test]
    fn augments_toml_is_consistent() {
        let (f, idx) = &*DATA;
        assert!(f.augment.len() >= 60, "{}", f.augment.len());
        assert_eq!(idx.len(), f.augment.len(), "duplicate ids");
        for a in &f.augment {
            assert!(!a.tags.is_empty() || !a.stats.is_empty(), "{} has neither tags nor stats", a.name);
            for (k, v) in &a.stats {
                assert!(STAT_KEYS.contains(&k.as_str()) && *v > 0.0, "{}: {k} = {v}", a.name);
            }
            assert!(!a.keep_slot || a.tags.contains(&Tag::QuestItem), "{}", a.name);
        }
        assert_eq!(get(2132).unwrap().name, "Warlock Juicebox");
        assert_eq!(get(1156).unwrap().requires, [3089, 3157]);
    }

    /// Names must match the u.gg manifest for the same ids (ids are shared with CommunityDragon).
    #[test]
    fn ids_match_ugg_manifest_names() {
        let path = format!("{}/../../fixtures/data/ugg-augment-manifest-16_21.json", env!("CARGO_MANIFEST_DIR"));
        let m: HashMap<String, String> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for a in &DATA.0.augment {
            assert_eq!(m.get(&a.id.to_string()), Some(&a.name), "id {}", a.id);
        }
    }

    #[test]
    fn unresolved_pick_uses_shared_tags_only() {
        let p = MyAugment { id: None, candidates: vec![2016, 1087], time: 0.0 }; // Juiced, Typhoon
        let e = AugEffects::new(&[p]);
        assert_eq!(e.sources[0].tags, vec![Tag::OnHit]);
        assert_eq!(e.sources[0].name, "your on-hit augment");
        let mixed = MyAugment { id: None, candidates: vec![2016, 1068], time: 0.0 }; // on-hit vs haste
        assert!(AugEffects::new(&[mixed]).sources.is_empty());
        let unknown = MyAugment { id: None, candidates: vec![2016, 99_999], time: 0.0 };
        assert!(AugEffects::new(&[unknown]).sources.is_empty());
    }
}
