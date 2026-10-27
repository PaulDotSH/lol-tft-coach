mod board;
mod capture;
mod engine;
#[cfg(test)]
mod tests;

pub use board::{capture_client, client_rect, client_size};
pub use capture::capture_league;
pub use engine::{recognize, recognize_boxes};
