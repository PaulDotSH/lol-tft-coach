//! Data Dragon items/champions + bundled `traits.toml` → [`StaticData`].

use anyhow::Context;
use model::{ChampClass, ChampKey, Champion, Item, ItemId, ItemStats, Profile, StaticData, Traits};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

const BASE: &str = "https://ddragon.leagueoflegends.com";
const TRAITS: &str = include_str!("../traits.toml");

/// Latest version, items and champions (cached per version).
pub async fn load(http: &crate::Http) -> anyhow::Result<StaticData> {
    let hour = Duration::from_secs(3600);
    let version = http.get_parsed(&format!("{BASE}/api/versions.json"), 6 * hour, parse_version).await?;
    let file = |name: &str| format!("{BASE}/cdn/{version}/data/en_US/{name}.json");
    // Per-version files never change.
    let items = http.get_parsed(&file("item"), 720 * hour, parse_items).await?;
    let champs = http.get_parsed(&file("champion"), 720 * hour, parse_champions).await?;
    Ok(StaticData { version, items, champs })
}

pub fn parse_version(json: &str) -> anyhow::Result<String> {
    let versions: Vec<String> = serde_json::from_str(json)?;
    versions.into_iter().next().context("empty versions.json")
}

// ---- items ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DItem {
    name: String,
    description: String,
    #[serde(default)]
    from: Vec<String>,
    #[serde(default)]
    into: Vec<String>,
    gold: Gold,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    maps: HashMap<String, bool>,
    #[serde(default)]
    stats: HashMap<String, f32>,
    depth: Option<u32>,
    required_champion: Option<String>,
    required_ally: Option<String>,
    in_store: Option<bool>,
    hide_from_all: Option<bool>,
    special_recipe: Option<ItemId>,
}

#[derive(Deserialize)]
struct Gold {
    total: u32,
    purchasable: bool,
}

impl DItem {
    fn rift(&self) -> bool {
        self.maps.get("11") == Some(&true)
    }
    fn aram(&self) -> bool {
        self.maps.get("12") == Some(&true)
    }
    fn tag(&self, t: &str) -> bool {
        self.tags.iter().any(|x| x == t)
    }
}

/// Rift/ARAM shop items. Ids ≥ 200000 are other modes' copies (Arena, event
/// modes); 1xxxxx are ARAM-only variants that lolalytics does report.
pub fn parse_items(json: &str) -> anyhow::Result<HashMap<ItemId, Item>> {
    #[derive(Deserialize)]
    struct File {
        data: HashMap<String, DItem>,
    }
    let raw: HashMap<ItemId, DItem> =
        serde_json::from_str::<File>(json)?.data.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
    let shop = |id: ItemId, d: &DItem| {
        id < 200_000
            && d.gold.purchasable
            && (d.rift() || d.aram())
            && d.required_champion.is_none()
            && d.required_ally.is_none()
            && d.in_store != Some(false)
            && d.hide_from_all != Some(true)
            && !d.tag("Trinket")
    };
    let mut keep: HashSet<ItemId> = raw.iter().filter(|(id, d)| shop(**id, d)).map(|(id, _)| *id).collect();
    // Transforms (Seraph's, Muramana, …) aren't purchasable but show up in live games.
    let transforms: Vec<ItemId> =
        raw.iter().filter(|(_, d)| d.special_recipe.is_some_and(|b| keep.contains(&b))).map(|(id, _)| *id).collect();
    keep.extend(transforms);

    let ids = |v: &[String]| -> Vec<ItemId> {
        v.iter().filter_map(|s| s.parse().ok()).filter(|i| keep.contains(i)).collect()
    };
    let boot_tag = |s: &String| s.parse().ok().and_then(|i: ItemId| raw.get(&i)).is_some_and(|d| d.tag("Boots"));
    Ok(keep
        .iter()
        .map(|&id| {
            let d = &raw[&id];
            let stats = parse_stats(&d.description).unwrap_or_else(|| stats_from_ddragon(&d.stats));
            // Tier-3 boots don't always carry the tag (Gunmetal Greaves).
            let boots = (d.tag("Boots") && !d.from.is_empty()) || d.from.iter().any(boot_tag);
            let into = ids(&d.into);
            let completed = d.special_recipe.is_none()
                && !d.tag("Consumable")
                && if boots { d.depth == Some(2) } else { into.is_empty() && d.gold.total >= 1400 };
            let item = Item {
                id,
                name: d.name.clone(),
                cost: d.gold.total,
                from: ids(&d.from),
                into,
                answers: answers(id, &stats, &d.description, &d.tags),
                stats,
                boots,
                completed,
                rift: d.rift(),
                aram: d.aram(),
                tags: d.tags.clone(),
            };
            (id, item)
        })
        .collect())
}

/// Parse the `<stats>` block of an item description (the only place lethality,
/// % pen and haste appear). Percentages are stored as fractions (30% → 0.30);
/// % move speed is folded into flat assuming a ~345 base.
pub fn parse_stats(desc: &str) -> Option<ItemStats> {
    let start = desc.find("<stats>")? + "<stats>".len();
    let block = &desc[start..start + desc[start..].find("</stats>")?];
    let mut s = ItemStats::default();
    for part in block.split("<attention>").skip(1) {
        let Some((value, rest)) = part.split_once("</attention>") else { continue };
        let pct = value.trim_end().ends_with('%');
        let Ok(mut v) = value.trim().trim_end_matches('%').parse::<f32>() else { continue };
        if pct {
            v /= 100.0;
        }
        let label = rest.split('<').next().unwrap_or("").trim();
        let field = match (label, pct) {
            ("Attack Damage", _) => &mut s.ad,
            ("Ability Power", _) => &mut s.ap,
            ("Armor", _) => &mut s.armor,
            ("Magic Resist", _) => &mut s.mr,
            ("Health", false) => &mut s.hp,
            ("Mana", false) => &mut s.mana,
            ("Attack Speed", _) => &mut s.attack_speed,
            ("Critical Strike Chance", _) => &mut s.crit,
            ("Lethality", _) => &mut s.lethality,
            ("Armor Penetration", _) => &mut s.armor_pen_pct,
            ("Magic Penetration", false) => &mut s.magic_pen_flat,
            ("Magic Penetration", true) => &mut s.magic_pen_pct,
            ("Ability Haste", _) => &mut s.ability_haste,
            ("Life Steal", _) => &mut s.lifesteal,
            ("Omnivamp", _) => &mut s.omnivamp,
            ("Move Speed", false) => &mut s.move_speed,
            ("Move Speed", true) => {
                v *= 345.0;
                &mut s.move_speed
            }
            _ => continue,
        };
        *field += v;
    }
    Some(s)
}

fn stats_from_ddragon(m: &HashMap<String, f32>) -> ItemStats {
    let g = |k: &str| m.get(k).copied().unwrap_or(0.0);
    ItemStats {
        ad: g("FlatPhysicalDamageMod"),
        ap: g("FlatMagicDamageMod"),
        armor: g("FlatArmorMod"),
        mr: g("FlatSpellBlockMod"),
        hp: g("FlatHPPoolMod"),
        mana: g("FlatMPPoolMod"),
        attack_speed: g("PercentAttackSpeedMod"),
        crit: g("FlatCritChanceMod"),
        lifesteal: g("PercentLifeStealMod"),
        move_speed: g("FlatMovementSpeedMod") + 345.0 * g("PercentMovementSpeedMod"),
        ..Default::default()
    }
}

/// Answers text parsing misses. Keep short.
type Override = (ItemId, fn(&mut Profile));
const OVERRIDES: &[Override] = &[
    (3302, |p| (p.armor_pen, p.magic_pen) = (0.5, 0.5)), // Terminus: stacking pen passive
    (8020, |p| p.magic_pen = 0.4),                       // Abyssal Mask: amplifies magic damage taken
];

/// What an item answers, 0..=1 per field, from stats + description keywords.
pub fn answers(id: ItemId, s: &ItemStats, desc: &str, tags: &[String]) -> Profile {
    let text = strip_tags(desc);
    let has = |k: &str| text.contains(k);
    let pick = |rules: &[(bool, f32)]| rules.iter().filter(|r| r.0).map(|r| r.1).fold(0.0, f32::max);
    let tenacity_stat = parse_tenacity(desc);
    let mut p = Profile {
        armor: (s.armor / 80.0).min(1.0),
        mr: (s.mr / 80.0).min(1.0),
        hp: (s.hp / 800.0).min(1.0),
        anti_heal: pick(&[(has("Wounds"), 1.0)]),
        tenacity: pick(&[(true, (tenacity_stat / 0.3).min(1.0)), (tags.iter().any(|t| t == "Tenacity"), 0.5)]),
        cleanse: pick(&[(has("Remove all crowd control") || has("Removes all crowd control"), 1.0)]),
        anti_burst: pick(&[
            (has("Stasis") || has("lethal damage"), 1.0),
            (has("Spell Shield"), 0.7),
            (has("Lifeline"), 0.6),
            (has("magic shield"), 0.4),
        ]),
        armor_pen: pick(&[
            (true, (s.lethality / 20.0 + s.armor_pen_pct / 0.35).min(1.0)),
            (has("reduces the target's Armor"), 0.6),
        ]),
        magic_pen: pick(&[
            (true, (s.magic_pen_flat / 20.0 + s.magic_pen_pct / 0.4).min(1.0)),
            (has("reduces their Magic Resist"), 0.6),
        ]),
        anti_crit: pick(&[(has("damage from Critical Strikes"), 1.0)]),
        anti_attack_speed: pick(&[(has("Reduce the Attack Speed"), 1.0), (has("damage from Attacks"), 0.4)]),
    };
    for (oid, f) in OVERRIDES {
        if *oid == id {
            f(&mut p);
        }
    }
    p
}

/// Description text without markup, for keyword checks.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn parse_tenacity(desc: &str) -> f32 {
    let Some(i) = desc.find("</attention> Tenacity") else { return 0.0 };
    let head = &desc[..i];
    head[head.rfind("<attention>").map_or(0, |j| j + "<attention>".len())..]
        .trim()
        .trim_end_matches('%')
        .parse::<f32>()
        .map_or(0.0, |v| v / 100.0)
}

// ---- champions ----

#[derive(Deserialize)]
struct DChamp {
    id: String,
    key: String,
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    info: Info,
}

#[derive(Deserialize)]
struct Info {
    attack: f32,
    magic: f32,
}

pub fn parse_champions(json: &str) -> anyhow::Result<HashMap<ChampKey, Champion>> {
    #[derive(Deserialize)]
    struct File {
        data: HashMap<String, DChamp>,
    }
    let curated = parse_traits(TRAITS)?;
    serde_json::from_str::<File>(json)?
        .data
        .into_values()
        .map(|c| {
            let key = c.key.parse().with_context(|| format!("champion key {}", c.key))?;
            let mut traits = default_traits(&c);
            if let Some(over) = curated.get(&c.id) {
                apply_traits(&mut traits, over)?;
            }
            let classes = c.tags.iter().filter_map(|t| class(t)).collect();
            Ok((key, Champion { key, slug: slug(&c.id, &c.name), id: c.id, name: c.name, classes, traits }))
        })
        .collect()
}

/// lolalytics / u.gg slug: lowercase alphanumerics of the display name, except
/// where the site uses a short form (verified against lolalytics' champion map).
pub fn slug(id: &str, name: &str) -> String {
    match id {
        "MonkeyKing" => "wukong".into(),
        "Nunu" => "nunu".into(),
        "Renata" => "renata".into(),
        _ => name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase(),
    }
}

fn class(tag: &str) -> Option<ChampClass> {
    Some(match tag {
        "Assassin" => ChampClass::Assassin,
        "Fighter" => ChampClass::Fighter,
        "Mage" => ChampClass::Mage,
        "Marksman" => ChampClass::Marksman,
        "Support" => ChampClass::Support,
        "Tank" => ChampClass::Tank,
        _ => return None,
    })
}

fn default_traits(c: &DChamp) -> Traits {
    let has = |t: &str| c.tags.iter().any(|x| x == t);
    let (a, m) = (c.info.attack.powi(2), c.info.magic.powi(2));
    Traits {
        magic_share: if a + m > 0.0 { m / (a + m) } else { 0.5 },
        true_damage: 0.0,
        heals: 0.0,
        cc: if has("Tank") {
            0.6
        } else if has("Support") {
            0.5
        } else {
            0.25
        },
        burst: if has("Assassin") {
            0.8
        } else if has("Mage") {
            0.5
        } else {
            0.2
        },
        on_hit: if has("Marksman") { 0.5 } else { 0.0 },
        tank: if has("Tank") {
            0.8
        } else if has("Fighter") {
            0.4
        } else {
            0.1
        },
    }
}

pub fn parse_traits(toml_src: &str) -> anyhow::Result<HashMap<String, HashMap<String, f32>>> {
    Ok(toml::from_str(toml_src)?)
}

fn apply_traits(t: &mut Traits, over: &HashMap<String, f32>) -> anyhow::Result<()> {
    for (k, &v) in over {
        let f = match k.as_str() {
            "magic_share" => &mut t.magic_share,
            "true_damage" => &mut t.true_damage,
            "heals" => &mut t.heals,
            "cc" => &mut t.cc,
            "burst" => &mut t.burst,
            "on_hit" => &mut t.on_hit,
            "tank" => &mut t.tank,
            _ => anyhow::bail!("traits.toml: unknown field {k}"),
        };
        *f = v.clamp(0.0, 1.0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = include_str!("../../../fixtures/data/ddragon-item.json");
    const CHAMPS: &str = include_str!("../../../fixtures/data/ddragon-champion.json");

    #[test]
    fn items() {
        let items = parse_items(ITEMS).unwrap();
        let it = |id: ItemId| &items[&id];

        let zhonya = it(3157);
        assert!(zhonya.completed && zhonya.rift && zhonya.aram && !zhonya.boots);
        assert_eq!((zhonya.stats.ap, zhonya.stats.armor, zhonya.cost), (105.0, 50.0, 3250));
        assert_eq!(zhonya.answers.anti_burst, 1.0);
        assert!(zhonya.from.contains(&2420));

        let youmuu = it(3142);
        assert_eq!(youmuu.stats.lethality, 18.0);
        assert!(youmuu.answers.armor_pen > 0.8);
        assert!((it(6694).stats.armor_pen_pct - 0.35).abs() < 1e-6);
        assert!((it(3135).stats.magic_pen_pct - 0.40).abs() < 1e-6);
        assert_eq!(it(4645).stats.magic_pen_flat, 15.0);
        assert_eq!(it(3110).stats.ability_haste, 20.0);

        assert_eq!(it(3033).answers.anti_heal, 1.0);
        assert_eq!(it(3165).answers.anti_heal, 1.0);
        assert_eq!(it(3075).answers.anti_heal, 1.0);
        assert_eq!(it(3140).answers.cleanse, 1.0);
        assert!(!it(3140).completed, "QSS is a component");
        assert_eq!(it(3139).answers.cleanse, 1.0);
        assert_eq!(it(3143).answers.anti_crit, 1.0);
        assert_eq!(it(3110).answers.anti_attack_speed, 1.0);
        assert_eq!(it(3026).answers.anti_burst, 1.0);
        assert_eq!(it(3102).answers.anti_burst, 0.7);
        assert_eq!(it(3053).answers.anti_burst, 0.6);
        assert_eq!(it(3111).answers.tenacity, 1.0);
        assert!(it(3071).answers.armor_pen >= 0.6, "Black Cleaver carve");
        assert_eq!(it(3302).answers.magic_pen, 0.5);
        assert_eq!(it(3075).answers.armor, 75.0 / 80.0);

        let merc = it(3111);
        assert!(merc.boots && merc.completed);
        assert!(!it(1001).completed && !it(1001).boots);
        assert!(it(3175).boots && !it(3175).completed, "tier-3 boots are upgrades");
        assert!(it(3172).boots, "Gunmetal Greaves lacks the Boots tag");
        assert!(it(3041).completed, "Mejai's");
        assert!(!it(1056).completed && !it(2003).completed && !it(3070).completed);
        assert!(it(2003).cost > 0, "consumables kept for start sets");
        assert!(it(3040).cost > 0 && !it(3040).completed, "Seraph's transform kept for gold value");
        assert!(it(126697).aram && !it(126697).rift);

        for id in [3330, 3599, 3901, 3340, 3364, 663056, 771001, 223084] {
            assert!(!items.contains_key(&id), "{id} should be excluded");
        }
        assert!(items.values().all(|i| i.rift || i.aram));
    }

    #[test]
    fn champions() {
        let champs = parse_champions(CHAMPS).unwrap();
        let by_id = |id: &str| champs.values().find(|c| c.id == id).unwrap();
        for (id, s) in [
            ("MonkeyKing", "wukong"),
            ("Nunu", "nunu"),
            ("Renata", "renata"),
            ("Kaisa", "kaisa"),
            ("Belveth", "belveth"),
            ("DrMundo", "drmundo"),
            ("Chogath", "chogath"),
            ("JarvanIV", "jarvaniv"),
            ("KSante", "ksante"),
        ] {
            assert_eq!(by_id(id).slug, s);
        }
        let ahri = &champs[&103];
        assert_eq!((ahri.id.as_str(), ahri.name.as_str()), ("Ahri", "Ahri"));
        assert_eq!(ahri.classes, vec![ChampClass::Mage, ChampClass::Assassin]);
        assert_eq!(ahri.traits.magic_share, 1.0);
        assert_eq!(by_id("Soraka").traits.heals, 1.0);
        assert!(by_id("Vayne").traits.true_damage > 0.5);
        assert!(by_id("Garen").traits.magic_share < 0.1, "tag default: physical");
        assert!(by_id("Malphite").traits.tank == 1.0);
    }

    #[test]
    fn traits_toml_names_real_champions() {
        let champs = parse_champions(CHAMPS).unwrap();
        let ids: HashSet<_> = champs.values().map(|c| c.id.as_str()).collect();
        for id in parse_traits(TRAITS).unwrap().keys() {
            assert!(ids.contains(id.as_str()), "traits.toml: unknown champion {id}");
        }
    }

    #[test]
    fn version() {
        assert_eq!(parse_version(r#"["16.21.1","16.20.1"]"#).unwrap(), "16.21.1");
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_load() {
        let http = crate::Http::new(std::env::temp_dir().join("lol-coach-live-cache")).unwrap();
        let sd = load(&http).await.unwrap();
        assert!(sd.items.len() > 150 && sd.champs.len() > 160, "{} items {} champs", sd.items.len(), sd.champs.len());
    }
}
