//! Read Mayhem augment offers off the League window: GDI capture of the
//! (borderless) game window → Windows.Media.Ocr → fuzzy match to augment names.
//! Blocking; call from a blocking thread. Never sends input or takes focus.

use model::AugmentId;
use std::collections::HashMap;

/// BGRA8 pixels, top-down.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Capture the region of the League game window where augment cards appear.
/// None when the game window is missing or minimised.
pub fn capture_league() -> Option<Frame> {
    todo!()
}

/// OCR text lines from a frame.
pub fn recognize(frame: &Frame) -> anyhow::Result<Vec<String>> {
    let _ = frame;
    todo!()
}

/// Augments whose names appear in `lines` (fuzzy), in screen order, deduped.
pub fn match_augments(lines: &[String], names: &HashMap<AugmentId, String>) -> Vec<AugmentId> {
    let _ = (lines, names);
    todo!()
}

/// capture → recognize → match. Empty when nothing is offered.
pub fn scan(names: &HashMap<AugmentId, String>) -> anyhow::Result<Vec<AugmentId>> {
    match capture_league() {
        Some(f) => Ok(match_augments(&recognize(&f)?, names)),
        None => Ok(vec![]),
    }
}
