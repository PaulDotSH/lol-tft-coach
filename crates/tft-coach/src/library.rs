//! What each unit looks like on my board: image embeddings of pieces named for sure
//! (bought, or forced by the trait panel), kept across games so a unit I didn't see
//! bought (carousel, a starting unit, one held when the app started) is known by its looks.
//! The app keeps it on disk (`to_bytes` / `from_bytes`).

use tft_model::Api;

/// Looks kept per unit (oldest dropped first).
const PER_UNIT: usize = 40;
/// A look this alike to one already kept for the unit adds nothing.
const DUPLICATE: f32 = 0.95;
const MAGIC: &[u8; 4] = b"TFL1";

#[derive(Debug, Clone, Default)]
pub struct Library {
    /// (unit, ★, embedding), oldest first.
    looks: Vec<(Api, u32, Vec<f32>)>,
}

pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

impl Library {
    pub fn len(&self) -> usize {
        self.looks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.looks.is_empty()
    }

    /// Units known by their looks.
    pub fn units(&self) -> usize {
        let mut u: Vec<&Api> = self.looks.iter().map(|l| &l.0).collect();
        u.sort();
        u.dedup();
        u.len()
    }

    /// Every known unit with its best similarity to `emb`, best first.
    pub fn classify(&self, emb: &[f32]) -> Vec<(Api, f32)> {
        let mut best: Vec<(Api, f32)> = vec![];
        for (api, _, e) in &self.looks {
            let s = similarity(emb, e);
            match best.iter_mut().find(|b| &b.0 == api) {
                Some(b) => b.1 = b.1.max(s),
                None => best.push((api.clone(), s)),
            }
        }
        best.sort_by(|a, b| b.1.total_cmp(&a.1));
        best
    }

    /// Keep a look of `api` at ★`star`; false when it's a near copy of one already kept.
    pub fn learn(&mut self, api: &str, star: u32, emb: &[f32]) -> bool {
        if emb.is_empty() || self.looks.iter().any(|(a, _, e)| a == api && similarity(e, emb) >= DUPLICATE) {
            return false;
        }
        if self.looks.iter().filter(|l| l.0 == api).count() >= PER_UNIT
            && let Some(i) = self.looks.iter().position(|l| l.0 == api)
        {
            self.looks.remove(i);
        }
        self.looks.push((api.into(), star, emb.to_vec()));
        true
    }

    /// "TFL1", then per look: name length (u16), name, ★ (u8), dimension (u16), f32s; little endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        for (api, star, e) in &self.looks {
            b.extend((api.len() as u16).to_le_bytes());
            b.extend(api.as_bytes());
            b.push(*star as u8);
            b.extend((e.len() as u16).to_le_bytes());
            e.iter().for_each(|v| b.extend(v.to_le_bytes()));
        }
        b
    }

    /// None when the bytes aren't a library (an older format, a cut-off write).
    pub fn from_bytes(b: &[u8]) -> Option<Library> {
        let mut rest = b.strip_prefix(MAGIC)?;
        let mut take = |n: usize| -> Option<&[u8]> {
            let (a, r) = rest.split_at_checked(n)?;
            rest = r;
            Some(a)
        };
        let mut looks = vec![];
        while let Some(n) = take(2) {
            let n = u16::from_le_bytes([n[0], n[1]]) as usize;
            let api = String::from_utf8(take(n)?.to_vec()).ok()?;
            let star = take(1)?[0] as u32;
            let d = take(2)?;
            let d = u16::from_le_bytes([d[0], d[1]]) as usize;
            let e = take(d * 4)?.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            looks.push((api, star, e));
        }
        Some(Library { looks })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: &[f32]) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / n).collect()
    }

    #[test]
    fn learns_classifies_and_round_trips() {
        let mut l = Library::default();
        assert!(l.learn("A", 1, &unit(&[1.0, 0.0, 0.0])));
        assert!(!l.learn("A", 1, &unit(&[1.0, 0.01, 0.0])));
        assert!(l.learn("B", 2, &unit(&[0.0, 1.0, 0.0])));
        let c = l.classify(&unit(&[0.9, 0.1, 0.0]));
        assert_eq!(c[0].0, "A");
        let back = Library::from_bytes(&l.to_bytes()).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.classify(&unit(&[0.0, 1.0, 0.1]))[0].0, "B");
        assert!(Library::from_bytes(b"nope").is_none());
    }
}
