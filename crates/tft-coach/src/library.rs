//! What each unit looks like: image embeddings of my pieces named for sure (bought, or forced
//! by the trait panel), kept across games so a unit I didn't see bought (carousel, a starting
//! unit, one held when the app started) is known by its looks; plus, built in, a seed of every
//! unit's looks from 3D renders (`with_seed`). Seed looks name other players' units only (see
//! `board::by_looks` and `scout`), are never learnt over and never saved. The app keeps the
//! learnt looks on disk (`to_bytes` / `from_bytes`). A classifier head (`with_head`, built in
//! once one passes the gate) names other players' units from the same embeddings; without one
//! (an empty file) that naming stays off.

use crate::head::Head;
use half::f16;
use std::collections::HashMap;
use std::sync::OnceLock;
use tft_model::Api;

/// Learnt looks kept per unit (oldest dropped first).
const PER_UNIT: usize = 40;
/// A look this alike to one already learnt for the unit adds nothing.
const DUPLICATE: f32 = 0.95;
/// f32 values, no seed flag (still read).
const MAGIC_V1: &[u8; 4] = b"TFL1";
/// f16 values and a seed flag.
const MAGIC: &[u8; 4] = b"TFL2";

#[derive(Debug, Clone)]
struct Look {
    api: Api,
    star: u32,
    /// Built in (renders): names units but isn't learnt over or saved.
    seed: bool,
    emb: Vec<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct Library {
    /// Seed and learnt looks; learnt ones oldest first.
    looks: Vec<Look>,
    /// What `classify_centered` and `units` derive from the learnt looks, made on first use and
    /// dropped when a look is learnt.
    learnt: OnceLock<Learnt>,
    head: Option<Head>,
}

#[derive(Debug, Clone)]
struct Learnt {
    /// Units known by learnt looks.
    units: usize,
    /// The first learnt look's dimension (None: nothing learnt).
    d: Option<usize>,
    /// (look index, learnt look less the mean of them all, at unit length) for looks of dimension `d`.
    centred: Vec<(usize, Vec<f32>)>,
}

pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The best unit among `c`'s seed (`seed`) or learnt matches when it is at least `sim` alike and
/// `margin` ahead of the next unit of the same kind.
pub fn standout(c: &[(Api, f32, bool)], seed: bool, sim: f32, margin: f32) -> Option<&Api> {
    let mut kind = c.iter().filter(|m| m.2 == seed);
    let (api, s, _) = kind.next()?;
    let next = kind.next().map_or(0.0, |n| n.1);
    (*s >= sim && s - next >= margin).then_some(api)
}

impl Library {
    /// Learnt looks (what `to_bytes` saves).
    pub fn len(&self) -> usize {
        self.looks.iter().filter(|l| !l.seed).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Units known by learnt looks.
    pub fn units(&self) -> usize {
        self.derived().units
    }

    fn derived(&self) -> &Learnt {
        self.learnt.get_or_init(|| {
            let learnt: Vec<(usize, &Look)> = self.looks.iter().enumerate().filter(|(_, l)| !l.seed).collect();
            let d = learnt.first().map(|(_, l)| l.emb.len());
            let mut centred = vec![];
            if let Some(d) = d {
                let mut mean = vec![0.0f32; d];
                for (_, l) in learnt.iter().filter(|(_, l)| l.emb.len() == d) {
                    mean.iter_mut().zip(&l.emb).for_each(|(m, v)| *m += v / learnt.len() as f32);
                }
                centred = learnt.iter().filter(|(_, l)| l.emb.len() == d).map(|&(i, l)| (i, unit(l.emb.iter().zip(&mean).map(|(a, b)| a - b).collect()))).collect();
            }
            Learnt { units: self.count_units(false), d, centred }
        })
    }

    /// Seed looks and the units they cover.
    pub fn seeded(&self) -> (usize, usize) {
        (self.looks.iter().filter(|l| l.seed).count(), self.count_units(true))
    }

    fn count_units(&self, seed: bool) -> usize {
        let mut u: Vec<&Api> = self.looks.iter().filter(|l| l.seed == seed).map(|l| &l.api).collect();
        u.sort();
        u.dedup();
        u.len()
    }

    /// The seed's looks added (a TFL1/TFL2 library; every look marked seed). Empty or unreadable
    /// bytes add nothing.
    pub fn with_seed(mut self, bytes: &[u8]) -> Library {
        if let Some(seed) = Library::from_bytes(bytes) {
            self.looks.extend(seed.looks.into_iter().map(|l| Look { seed: true, ..l }));
            self.learnt = OnceLock::new();
        }
        self
    }

    /// The classifier head added (`head.bin`); empty or unreadable bytes leave it off.
    pub fn with_head(mut self, bytes: &[u8]) -> Library {
        self.head = Head::from_bytes(bytes);
        self
    }

    pub fn has_head(&self) -> bool {
        self.head.is_some()
    }

    /// The head's probability per unit for `emb`, best first (empty without a head).
    pub fn classify_head(&self, emb: &[f32]) -> Vec<(Api, f32)> {
        self.head.as_ref().map_or(vec![], |h| h.scores(emb))
    }

    /// The unit the head names for `emb` at the thresholds stored with it.
    pub fn head_name(&self, emb: &[f32]) -> Option<Api> {
        self.head.as_ref()?.name(emb)
    }

    /// Every known unit with its best similarity to `emb`, best first: (unit, similarity, seed).
    /// A unit's best learnt and best seed look are kept apart (up to two entries per unit).
    pub fn classify(&self, emb: &[f32]) -> Vec<(Api, f32, bool)> {
        let mut best: Vec<(Api, f32, bool)> = vec![];
        // Where each (unit, seed) sits in `best`, which stays in first-seen order.
        let mut at: HashMap<(&str, bool), usize> = HashMap::new();
        for l in &self.looks {
            let s = similarity(emb, &l.emb);
            match at.get(&(l.api.as_str(), l.seed)) {
                Some(&i) => best[i].1 = best[i].1.max(s),
                None => {
                    at.insert((l.api.as_str(), l.seed), best.len());
                    best.push((l.api.clone(), s, l.seed));
                }
            }
        }
        best.sort_by(|a, b| b.1.total_cmp(&a.1));
        best
    }

    /// Like `classify` on learnt looks only, with the background each side shares taken out: the
    /// learnt looks less their mean (my board's floor), `emb` less `center` (the mean of its own
    /// read: another arena's floor). For units seen away from my board. Seed looks are left out:
    /// they're composited on both arenas, so they're compared as is.
    pub fn classify_centered(&self, emb: &[f32], center: &[f32]) -> Vec<(Api, f32)> {
        let learnt = self.derived();
        if learnt.d.is_none_or(|d| d != emb.len() || d != center.len()) {
            return vec![];
        }
        let q = unit(emb.iter().zip(center).map(|(a, b)| a - b).collect());
        let mut best: Vec<(Api, f32)> = vec![];
        let mut at: HashMap<&str, usize> = HashMap::new();
        for (i, c) in &learnt.centred {
            let (api, s) = (&self.looks[*i].api, similarity(&q, c));
            match at.get(api.as_str()) {
                Some(&k) => best[k].1 = best[k].1.max(s),
                None => {
                    at.insert(api, best.len());
                    best.push((api.clone(), s));
                }
            }
        }
        best.sort_by(|a, b| b.1.total_cmp(&a.1));
        best
    }

    /// Keep a look of `api` at ★`star`; false when it's a near copy of one already learnt (seed
    /// looks don't count, and are never dropped for it).
    pub fn learn(&mut self, api: &str, star: u32, emb: &[f32]) -> bool {
        let mine = |l: &Look| !l.seed && l.api == api;
        if emb.is_empty() || self.looks.iter().any(|l| mine(l) && similarity(&l.emb, emb) >= DUPLICATE) {
            return false;
        }
        if self.looks.iter().filter(|l| mine(l)).count() >= PER_UNIT
            && let Some(i) = self.looks.iter().position(mine)
        {
            self.looks.remove(i);
        }
        self.looks.push(Look { api: api.into(), star, seed: false, emb: emb.to_vec() });
        self.learnt = OnceLock::new();
        true
    }

    /// The learnt looks: "TFL2", then per look: name length (u16), name, ★ (u8), seed (u8),
    /// dimension (u16), f16s; little endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        encode(self.looks.iter().filter(|l| !l.seed))
    }

    /// TFL2 or TFL1 (f32s, no seed byte). None when the bytes aren't a library (another format,
    /// a cut-off write).
    pub fn from_bytes(b: &[u8]) -> Option<Library> {
        let (v2, mut rest) = match (b.strip_prefix(MAGIC), b.strip_prefix(MAGIC_V1)) {
            (Some(r), _) => (true, r),
            (_, Some(r)) => (false, r),
            _ => return None,
        };
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
            let seed = v2 && take(1)?[0] != 0;
            let d = take(2)?;
            let d = u16::from_le_bytes([d[0], d[1]]) as usize;
            let emb = match v2 {
                true => take(d * 2)?.chunks(2).map(|c| f16::from_le_bytes([c[0], c[1]]).to_f32()).collect(),
                false => take(d * 4)?.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
            };
            looks.push(Look { api, star, seed, emb });
        }
        Some(Library { looks, learnt: OnceLock::new(), head: None })
    }
}

/// `v` at unit length.
fn unit(v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    v.into_iter().map(|x| x / n).collect()
}

fn encode<'a>(looks: impl Iterator<Item = &'a Look>) -> Vec<u8> {
    let mut b = MAGIC.to_vec();
    for l in looks {
        b.extend((l.api.len() as u16).to_le_bytes());
        b.extend(l.api.as_bytes());
        b.push(l.star as u8);
        b.push(l.seed as u8);
        b.extend((l.emb.len() as u16).to_le_bytes());
        l.emb.iter().for_each(|v| b.extend(f16::from_f32(*v).to_le_bytes()));
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: &[f32]) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / n).collect()
    }

    fn seed(looks: &[(&str, &[f32])]) -> Vec<u8> {
        let mut s = Library::default();
        for (api, v) in looks {
            s.learn(api, 1, &unit(v));
        }
        s.to_bytes()
    }

    #[test]
    fn learns_classifies_and_round_trips() {
        let mut l = Library::default();
        assert!(l.learn("A", 1, &unit(&[1.0, 0.0, 0.0])));
        assert!(!l.learn("A", 1, &unit(&[1.0, 0.01, 0.0])));
        assert!(l.learn("B", 2, &unit(&[0.0, 1.0, 0.0])));
        let c = l.classify(&unit(&[0.9, 0.1, 0.0]));
        assert_eq!((c[0].0.as_str(), c[0].2), ("A", false));
        let back = Library::from_bytes(&l.to_bytes()).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.classify(&unit(&[0.0, 1.0, 0.1]))[0].0, "B");
        assert!(Library::from_bytes(b"nope").is_none());
    }

    #[test]
    fn values_round_trip_as_f16_and_tfl1_still_loads() {
        let v: Vec<f32> = unit(&(0..768).map(|i| ((i * 37 % 101) as f32 - 50.0) / 7.0).collect::<Vec<_>>());
        let mut l = Library::default();
        l.learn("A", 2, &v);
        let bytes = l.to_bytes();
        let back = Library::from_bytes(&bytes).unwrap();
        assert!(back.looks[0].emb.iter().zip(&v).all(|(a, b)| (a - b).abs() <= 1e-3));
        assert_eq!((back.looks[0].star, back.looks[0].seed), (2, false));
        assert!(Library::from_bytes(&bytes[..bytes.len() - 1]).is_none());
        // TFL1: name length, name, ★, dimension, f32s.
        let mut old = MAGIC_V1.to_vec();
        old.extend(1u16.to_le_bytes());
        old.extend(b"B");
        old.push(3);
        old.extend(2u16.to_le_bytes());
        [0.6f32, 0.8].iter().for_each(|x| old.extend(x.to_le_bytes()));
        let old = Library::from_bytes(&old).unwrap();
        assert_eq!((old.len(), old.looks[0].star, old.looks[0].emb.clone()), (1, 3, vec![0.6, 0.8]));
    }

    #[test]
    fn the_seed_survives_learning_and_is_not_saved() {
        // Axis 0: A, 1: B, the rest: what tells A's learnt looks apart.
        let d = PER_UNIT + 8;
        let axes = |i: usize, j: usize| unit(&(0..d).map(|k| if k == i || k == j { 1.0 } else { 0.0 }).collect::<Vec<_>>());
        let mut l = Library::default().with_seed(&seed(&[("A", &axes(0, 0)), ("B", &axes(1, 1))]));
        assert_eq!((l.len(), l.units(), l.seeded()), (0, 0, (2, 2)));
        assert!(l.is_empty());
        // A look like the seed's is still learnt (seed looks don't dedupe), and many don't push the seed out.
        assert!(l.learn("A", 1, &axes(0, 0)));
        for i in 0..PER_UNIT + 5 {
            assert!(l.learn("A", 1, &axes(0, 2 + i % (d - 2))));
        }
        assert_eq!((l.len(), l.seeded()), (PER_UNIT, (2, 2)));
        let c = l.classify(&axes(0, 0));
        assert!(c.iter().any(|m| m.0 == "A" && m.2) && c.iter().any(|m| m.0 == "A" && !m.2));
        assert_eq!(standout(&c, true, 0.9, 0.1).map(|a| a.as_str()), Some("A"));
        // Saving keeps only what was learnt.
        let back = Library::from_bytes(&l.to_bytes()).unwrap();
        assert_eq!((back.len(), back.seeded()), (PER_UNIT, (0, 0)));
        // No seed: empty bytes (the placeholder file).
        assert_eq!(Library::default().with_seed(&[]).seeded(), (0, 0));
    }

    #[test]
    fn centred_looks_ignore_a_shared_background() {
        let mut l = Library::default();
        // My board's floor (axis 3) under every look; another arena's floor (axis 4) under the query.
        l.learn("A", 1, &unit(&[1.0, 0.0, 0.0, 2.0, 0.0]));
        l.learn("B", 1, &unit(&[0.0, 1.0, 0.0, 2.0, 0.0]));
        l.learn("C", 1, &unit(&[0.0, 0.0, 1.0, 2.0, 0.0]));
        let away = |v: &[f32]| unit(v);
        let read = [away(&[1.0, 0.0, 0.0, 0.0, 2.0]), away(&[0.0, 0.0, 1.0, 0.0, 2.0])];
        let center: Vec<f32> = (0..5).map(|i| (read[0][i] + read[1][i]) / 2.0).collect();
        assert_eq!(l.classify_centered(&read[0], &center)[0].0, "A");
        assert_eq!(l.classify_centered(&read[1], &center)[0].0, "C");
        // Seed looks change nothing here.
        let l = l.with_seed(&seed(&[("B", &[1.0, 0.0, 0.0, 0.0, 2.0])]));
        assert_eq!(l.classify_centered(&read[0], &center)[0].0, "A");
    }
}
