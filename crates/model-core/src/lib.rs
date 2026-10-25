//! Ids, small enums and the type macros every `model-*` crate uses. Changes here rebuild
//! every LoL crate, so only stable primitives live here.
//!
//! IDs: a champion's Riot numeric key is also the lolalytics `cid` and the u.gg id.
//!
//! TypeScript: the `TS` derive (bindings in `web/src/lib/bindings/`) is compiled only for tests
//! and with the `ts` feature, so app builds skip it. `cargo test -p model*` regenerates them.

pub type ChampKey = u32;
pub type ItemId = u32;
pub type AugmentId = u32;
/// Riot perk id: runes 8xxx/9xxx, stat shards 5xxx.
pub type RuneId = u32;

/// Plain data mirrored to TypeScript. The caller crate needs `serde` and, for tests or its
/// `ts` feature, `ts-rs`; field-level `ts(...)` attributes go behind the same `cfg_attr`.
#[macro_export]
macro_rules! data {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[cfg_attr(any(test, feature = "ts"), derive(::ts_rs::TS), ts(export, export_to = "../../../web/src/lib/bindings/"))]
        $item
    )*};
}

/// [`data!`] plus `Default`.
#[macro_export]
macro_rules! data_default {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, Default, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[cfg_attr(any(test, feature = "ts"), derive(::ts_rs::TS), ts(export, export_to = "../../../web/src/lib/bindings/"))]
        $item
    )*};
}

/// Fieldless enums, lowercase on the wire.
#[macro_export]
macro_rules! enums {
    ($($item:item)*) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        #[cfg_attr(any(test, feature = "ts"), derive(::ts_rs::TS), ts(export, export_to = "../../../web/src/lib/bindings/"))]
        $item
    )*};
}

enums! {
    /// Lolalytics lane names; Live API `UTILITY` = Support, `MIDDLE` = Middle.
    pub enum Lane { Top, Jungle, Middle, Bottom, Support }

    /// Data family, not the exact queue: ranked and normal draft share Rift stats.
    pub enum Mode { Rift, Aram, Mayhem }

    pub enum Team { Order, Chaos }

    pub enum ChampClass { Assassin, Fighter, Mage, Marksman, Support, Tank }

    pub enum Rarity { Prismatic, Gold, Silver }
}

impl Lane {
    pub const ALL: [Lane; 5] = [Lane::Top, Lane::Jungle, Lane::Middle, Lane::Bottom, Lane::Support];

    /// Lolalytics query value.
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::Top => "top",
            Lane::Jungle => "jungle",
            Lane::Middle => "middle",
            Lane::Bottom => "bottom",
            Lane::Support => "support",
        }
    }

    /// Live Client API `position` / LCU `assignedPosition` (case-insensitive).
    pub fn from_riot(s: &str) -> Option<Lane> {
        match s.to_ascii_lowercase().as_str() {
            "top" => Some(Lane::Top),
            "jungle" => Some(Lane::Jungle),
            "middle" | "mid" => Some(Lane::Middle),
            "bottom" | "bot" | "adc" => Some(Lane::Bottom),
            "utility" | "support" => Some(Lane::Support),
            _ => None,
        }
    }
}

impl Mode {
    /// LCU queue id → data family. 450 = ARAM, 2400 = ARAM: Mayhem.
    pub fn from_queue(queue_id: i64) -> Mode {
        match queue_id {
            450 | 100 => Mode::Aram,
            2400..=2499 => Mode::Mayhem,
            _ => Mode::Rift,
        }
    }
}

// Shared by in-game advice (buy next, path) and champ select (build plan).
data! {
    pub struct ItemPick {
        pub item: ItemId,
        pub name: String,
        pub cost: u32,
        pub score: f32,
        /// One line, e.g. "Zed is 7/1 and killed you 3×".
        pub reason: String,
        /// Best affordable component toward `item` when gold is short.
        pub component_now: Option<ItemId>,
    }
}
