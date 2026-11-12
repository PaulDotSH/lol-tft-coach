//! Screen reader health (Settings, "Screen reading"): per reader, whether it found its element
//! this session and where, and whether it missed a whole game it should have shown in. A miss
//! is logged once per game; the header's OCR / Minimap dot turns amber while it stands.

use model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    Offer,
    Scoreboard,
    Minimap,
}

const READERS: [Reader; 3] = [Reader::Offer, Reader::Scoreboard, Reader::Minimap];

impl Reader {
    pub fn name(self) -> &'static str {
        match self {
            Reader::Offer => "Augment offer",
            Reader::Scoreboard => "Tab scoreboard",
            Reader::Minimap => "Minimap",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Track {
    /// In use this game (or the last one, between games).
    enabled: bool,
    /// Its element should have shown this game.
    expected: bool,
    found_game: bool,
    /// Not found in the last game it should have shown in, and not found since.
    missed: bool,
    last_ok: Option<u64>,
    /// Where it was last found, when not where the default layout puts it.
    shift: Option<String>,
    /// Calibration last checked on this older patch.
    since: Option<String>,
}

#[derive(Debug, Default)]
pub struct Health {
    r: [Track; 3],
}

impl Health {
    fn t(&mut self, r: Reader) -> &mut Track {
        &mut self.r[r as usize]
    }

    pub fn start_game(&mut self) {
        for t in &mut self.r {
            (t.enabled, t.expected, t.found_game) = (false, false, false);
        }
    }

    pub fn enable(&mut self, r: Reader, on: bool) {
        self.t(r).enabled = on;
    }

    pub fn expect(&mut self, r: Reader) {
        self.t(r).expected = true;
    }

    /// Found at unix ms `now`; `shift` says how it differs from the default layout.
    pub fn found(&mut self, r: Reader, now: u64, shift: Option<String>) {
        let t = self.t(r);
        (t.found_game, t.missed, t.last_ok, t.shift, t.since) = (true, false, Some(now), shift, None);
    }

    /// The calibration for this window was last checked on `since` (None: on this patch, or never).
    pub fn calibrated(&mut self, r: Reader, since: Option<String>) {
        self.t(r).since = since;
    }

    /// The game ended: readers in use whose element should have shown and never was found are
    /// NotFound from now on. Returns them, to warn about once.
    pub fn end_game(&mut self) -> Vec<Reader> {
        let mut out = vec![];
        for (t, r) in self.r.iter_mut().zip(READERS) {
            if t.enabled && t.expected && !t.found_game {
                t.missed = true;
                out.push(r);
            }
        }
        out
    }

    pub fn report(&self) -> Vec<ReaderHealth> {
        self.r
            .iter()
            .zip(READERS)
            .map(|(t, r)| {
                let (state, detail) = if !t.enabled {
                    (ReaderState::Off, None)
                } else if t.missed {
                    (ReaderState::NotFound, Some("not found in the last game".to_string()))
                } else if t.last_ok.is_some() {
                    match &t.shift {
                        Some(s) => (ReaderState::Shifted, Some(s.clone())),
                        None => (ReaderState::Ok, None),
                    }
                } else {
                    (ReaderState::Unverified, Some(t.since.as_ref().map_or("not checked yet".to_string(), |p| format!("not checked since {p}"))))
                };
                ReaderHealth { name: r.name().to_string(), state, detail, last_ok: t.last_ok }
            })
            .collect()
    }
}

/// The header dots keep their meaning, amber while a reader behind them missed a whole game.
pub fn degrade(sources: &mut Sources, readers: &[ReaderHealth]) {
    let missed = |r: Reader| readers.iter().any(|h| h.name == r.name() && h.state == ReaderState::NotFound);
    for (dot, rs) in [(&mut sources.ocr, &[Reader::Offer, Reader::Scoreboard][..]), (&mut sources.minimap, &[Reader::Minimap][..])] {
        if *dot != SourceState::Error && rs.iter().any(|&r| missed(r)) {
            *dot = SourceState::Degraded;
        }
    }
}

/// The shift note for a reader found at `scale` of the default layout and `offset` (fraction of the
/// window) from its default place; None within tolerance.
pub fn shift(scale: f32, offset: f32) -> Option<String> {
    const SCALE_TOL: f32 = 0.03;
    const OFFSET_TOL: f32 = 0.02;
    match ((scale - 1.0).abs() > SCALE_TOL, offset.abs() > OFFSET_TOL) {
        (true, _) => Some(format!("found at {scale:.2}× (HUD scale)")),
        (false, true) => Some(format!("found {:.0} % from its usual place", offset.abs() * 100.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(h: &Health, r: Reader) -> (ReaderState, Option<String>) {
        let rep = h.report();
        let x = &rep[r as usize];
        assert_eq!(x.name, r.name());
        (x.state, x.detail.clone())
    }

    #[test]
    fn unverified_to_ok_on_the_first_find() {
        let mut h = Health::default();
        assert_eq!(state(&h, Reader::Offer).0, ReaderState::Off);
        h.start_game();
        h.enable(Reader::Offer, true);
        assert_eq!(state(&h, Reader::Offer), (ReaderState::Unverified, Some("not checked yet".into())));
        h.calibrated(Reader::Offer, Some("16.22".into()));
        assert_eq!(state(&h, Reader::Offer), (ReaderState::Unverified, Some("not checked since 16.22".into())));
        h.found(Reader::Offer, 5, None);
        assert_eq!(state(&h, Reader::Offer), (ReaderState::Ok, None));
        assert_eq!(h.report()[0].last_ok, Some(5));
        h.found(Reader::Offer, 6, shift(0.92, 0.0));
        assert_eq!(state(&h, Reader::Offer), (ReaderState::Shifted, Some("found at 0.92× (HUD scale)".into())));
    }

    #[test]
    fn ok_to_not_found_after_a_game_without_a_find() {
        let mut h = Health::default();
        h.start_game();
        h.enable(Reader::Minimap, true);
        h.found(Reader::Minimap, 1, None);
        assert!(h.end_game().is_empty());
        h.start_game();
        h.enable(Reader::Minimap, true);
        h.enable(Reader::Scoreboard, true);
        h.expect(Reader::Minimap);
        assert_eq!(state(&h, Reader::Minimap).0, ReaderState::Ok);
        assert_eq!(h.end_game(), [Reader::Minimap]);
        assert_eq!(state(&h, Reader::Minimap).0, ReaderState::NotFound);
        assert_eq!(state(&h, Reader::Scoreboard).0, ReaderState::Unverified, "not expected: no miss");
        let mut sources = Sources {
            client: SourceState::Ok,
            game: SourceState::Off,
            lolalytics: SourceState::Ok,
            ugg: SourceState::Ok,
            ocr: SourceState::Off,
            minimap: SourceState::Off,
            coachless: SourceState::Off,
            coachless_premium: false,
            message: None,
        };
        degrade(&mut sources, &h.report());
        assert_eq!((sources.minimap, sources.ocr), (SourceState::Degraded, SourceState::Off));
        h.start_game();
        h.enable(Reader::Minimap, true);
        assert_eq!(state(&h, Reader::Minimap).0, ReaderState::NotFound, "stands until found");
        h.found(Reader::Minimap, 9, None);
        assert_eq!(state(&h, Reader::Minimap).0, ReaderState::Ok);
    }

    #[test]
    fn shift_tolerances() {
        assert_eq!(shift(1.02, 0.005), None);
        assert_eq!(shift(1.0, 0.04).as_deref(), Some("found 4 % from its usual place"));
        assert_eq!(shift(1.2, 0.0).as_deref(), Some("found at 1.20× (HUD scale)"));
    }
}
