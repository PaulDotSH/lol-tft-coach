//! TFT data: Community Dragon Set 18 static data, MetaTFT meta stats and the LCU
//! TFT match history. Pure parsers (tested on `fixtures/tft/`) plus fetches through
//! the shared `data::Http` cache.

pub mod cdragon;
pub mod history;
pub mod metatft;

pub use cdragon::{icon_url, item_icons, parse_set_data, set_data};
pub use history::parse_history;
pub use metatft::{augment_tiers, meta};

/// The current set; `set_data` loads it.
pub const SET: u32 = 18;

/// Placement histogram (index 0 = 1st place) → games, average placement, top-4 share.
pub(crate) fn place_stat(api: &str, places: &[u32]) -> tft_model::PlaceStat {
    let places = &places[..places.len().min(8)];
    let games: u32 = places.iter().sum();
    let n = games.max(1) as f32;
    let avg = places.iter().enumerate().map(|(i, &c)| (i + 1) as f32 * c as f32).sum::<f32>() / n;
    let top4 = places.iter().take(4).sum::<u32>() as f32 / n;
    tft_model::PlaceStat { api: api.to_owned(), games, avg_place: if games == 0 { 0.0 } else { avg }, top4 }
}

#[cfg(test)]
pub(crate) fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../../fixtures/tft/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[cfg(test)]
pub(crate) fn live_http() -> data::Http {
    data::Http::new(std::env::temp_dir().join("lol-coach-live-cache")).unwrap()
}

#[cfg(test)]
mod tests {
    #[test]
    fn place_stat() {
        let s = super::place_stat("x", &[1, 1, 0, 0, 0, 0, 0, 2, 4]);
        assert_eq!((s.games, s.avg_place, s.top4), (4, 4.75, 0.5), "the 9th entry (comps_stats total) is ignored");
        assert_eq!(super::place_stat("y", &[0; 8]).avg_place, 0.0);
    }
}
