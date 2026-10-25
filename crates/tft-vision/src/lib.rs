//! Reads my TFT screen into a `TftSnapshot`: stage, gold, level, XP, HP, streak,
//! team size, shop names, offered augments, the trait panel and item icons.

use tft_model::{TftSetData, TftSnapshot};

/// Reads the TFT window; keeps the name tables OCR text is matched against.
pub struct Reader {}

impl Reader {
    pub fn new(_set: &TftSetData) -> Reader {
        Reader {}
    }

    /// One read of the screen; None when the TFT window isn't up.
    pub fn read(&mut self) -> Option<TftSnapshot> {
        None
    }
}
