mod board;
mod capture;
mod engine;
#[cfg(test)]
mod tests;

pub use board::{capture_client, client_rect, client_size};
pub use capture::{capture_league, league_foreground};
pub use engine::{recognize, recognize_boxes};
