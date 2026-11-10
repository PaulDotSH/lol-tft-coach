//! Fuzzy matching of OCR text against augment names.

use model_core::AugmentId;
use std::collections::HashMap;

const MAX_OFFERS: usize = 4;

/// Lowercase alphanumeric words with common OCR confusions and accents folded ("pământ"
/// reads as "pämånt"); "+" stays, it tells tiers apart ("Residual Magic" / "+" / "++").
/// Applied to both OCR text and names, so folding is symmetric.
fn words(s: &str) -> Vec<String> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            '\'' | '\u{2019}' | '`' => {}
            '0' => out.push('o'),
            '1' | '|' => out.push('l'),
            '+' => out.push('+'),
            'à'..='å' | 'ă' | 'ą' => out.push('a'),
            'è'..='ë' | 'ę' | 'ě' => out.push('e'),
            'ì'..='ï' => out.push('i'),
            'ò'..='ö' | 'ő' => out.push('o'),
            'ù'..='ü' | 'ű' | 'ů' => out.push('u'),
            'ș' | 'ş' | 'ś' | 'š' => out.push('s'),
            'ț' | 'ţ' => out.push('t'),
            'ç' | 'ć' | 'č' => out.push('c'),
            'ñ' | 'ń' => out.push('n'),
            'ž' | 'ź' | 'ż' => out.push('z'),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => out.push(' '),
        }
    }
    out.split_whitespace()
        .map(|w| w.replace("rn", "m"))
        .collect()
}

/// Edits allowed for a (space-free) name of `len` chars: short names must be
/// (near-)exact, longer ones tolerate ~15 %.
fn budget(len: usize) -> usize {
    match len {
        0..=4 => 0,
        5..=8 => 1,
        n => (n * 15 + 50) / 100,
    }
}

struct Target {
    id: AugmentId,
    key: String,
    words: usize,
}

/// A matched name: OCR words `start..end` (indices into [`ocr_words`]) and how well they match (1 = exact).
pub(crate) struct Hit {
    pub id: AugmentId,
    pub start: usize,
    pub end: usize,
    score: i64,
    pub quality: f32,
}

/// Folded OCR words with the line each came from.
pub(crate) fn ocr_words(lines: &[String]) -> Vec<(usize, String)> {
    lines.iter().enumerate().flat_map(|(i, l)| words(l).into_iter().map(move |w| (i, w))).collect()
}

/// Augments whose names appear in `lines` (fuzzy), in screen order, deduped. A name may
/// list translations one per line ("Take 'em Down\nPune-i la pământ"): any of them matches.
///
/// OCR words are concatenated (spaces dropped, so split/merged words don't
/// matter) over windows of the name's word count ±1, which may span one line
/// break (wrapped names). Overlapping hits are resolved in favour of the hit
/// explaining more characters.
pub fn match_augments(lines: &[String], names: &HashMap<AugmentId, String>) -> Vec<AugmentId> {
    hits(&ocr_words(lines), names).iter().map(|h| h.id).collect()
}

/// [`match_augments`] with the words each name was read from, in screen order.
pub(crate) fn hits(ocr: &[(usize, String)], names: &HashMap<AugmentId, String>) -> Vec<Hit> {
    let targets: Vec<Target> = names
        .iter()
        .flat_map(|(&id, name)| name.lines().map(move |n| (id, n)))
        .filter_map(|(id, name)| {
            let w = words(name);
            (!w.is_empty()).then(|| Target {
                id,
                key: w.concat(),
                words: w.len(),
            })
        })
        .collect();
    let max_words = targets.iter().map(|t| t.words).max().unwrap_or(0) + 1;

    let mut hits = Vec::new();
    let mut window = String::new();
    for start in 0..ocr.len() {
        window.clear();
        for end in start + 1..=(start + max_words).min(ocr.len()) {
            if ocr[end - 1].0 > ocr[start].0 + 1 {
                break;
            }
            window.push_str(&ocr[end - 1].1);
            let n = end - start;
            for t in &targets {
                let allowed = budget(t.key.len());
                if n.abs_diff(t.words) > 1 || window.len().abs_diff(t.key.len()) > allowed {
                    continue;
                }
                let d = strsim::levenshtein(&window, &t.key);
                if d <= allowed {
                    let score = t.key.len() as i64 - 2 * d as i64;
                    let quality = 1.0 - d as f32 / t.key.len().max(1) as f32;
                    hits.push(Hit { id: t.id, start, end, score, quality });
                }
            }
        }
    }

    hits.sort_by_key(|h| (std::cmp::Reverse(h.score), h.start));
    let mut taken: Vec<Hit> = Vec::new();
    for h in hits {
        if !taken
            .iter()
            .any(|t| t.id == h.id || (h.start < t.end && t.start < h.end))
        {
            taken.push(h);
        }
    }
    // Card titles are whole lines; a name inside a longer line is other text
    // (shop tooltips: "Invulnerability", "105 Ability Power"). Lines fully made of
    // names (titles merged by OCR) still count.
    let mut covered = vec![false; ocr.len()];
    for h in &taken {
        covered[h.start..h.end].fill(true);
    }
    let whole = |h: &Hit| {
        let lines = ocr[h.start].0..=ocr[h.end - 1].0;
        ocr.iter().zip(&covered).all(|((l, _), &c)| c || !lines.contains(l))
    };
    taken.retain(whole);
    taken.sort_by_key(|h| h.start);
    taken.truncate(MAX_OFFERS);
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> HashMap<AugmentId, String> {
        [
            (1, "Accelerating Sorcery"),
            (2, "Blade Waltz"),
            (3, "Courage of the Colossus"),
            (4, "Colossus"),
            (5, "Mad Scientist"),
            (6, "Dawnbringer's Resolve"),
            (7, "Ice Cold"),
            (8, "Erosion"),
            (9, "Back to Basics"),
            (10, "Goliath"),
            (11, "Infernal Conduit"),
            (12, "Firebrand"),
        ]
        .into_iter()
        .map(|(id, n)| (id, n.to_string()))
        .collect()
    }

    fn run(lines: &[&str]) -> Vec<AugmentId> {
        let lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        match_augments(&lines, &pool())
    }

    #[test]
    fn exact_and_noisy() {
        assert_eq!(
            run(&[
                "Acce1erating S0rcery",
                "Blade Wa1tz",
                "Courage of the Co1ossus"
            ]),
            [1, 2, 3]
        );
        assert_eq!(
            run(&[
                "Accelerafing Sorcerv",
                "BladeWaltz",
                "Courage ofthe Colosus"
            ]),
            [1, 2, 3]
        );
        assert_eq!(
            run(&["Dawnbringers Resolve", "lce Cold", "Erosi0n"]),
            [6, 7, 8]
        );
    }

    #[test]
    fn wrapped_and_merged_lines() {
        assert_eq!(
            run(&["Courage of the", "Colossus", "Mad", "Scientist"]),
            [3, 5]
        );
        assert_eq!(
            run(&["Accelerating Sorcery Blade Waltz Courage of the Colossus"]),
            [1, 2, 3]
        );
        assert_eq!(run(&["Select an augment", "Back to Basics", "Reroll"]), [9]);
    }

    #[test]
    fn plus_tiers_match_their_own_title() {
        let names: HashMap<AugmentId, String> = [(1, "Residual Magic"), (2, "Residual Magic +"), (3, "Residual Magic ++"), (4, "Kick Start")]
            .into_iter()
            .map(|(id, n)| (id, n.to_string()))
            .collect();
        let m = |l: &[&str]| match_augments(&l.iter().map(|s| s.to_string()).collect::<Vec<_>>(), &names);
        assert_eq!(m(&["Residual Magic", "Kick Start"]), [1, 4]);
        assert_eq!(m(&["Residual Magic +", "Kick Start"]), [2, 4]);
        assert_eq!(m(&["Residual Magic ++"]), [3]);
    }

    #[test]
    fn translated_names_with_misread_accents() {
        // A Romanian client's offer as OCR read it.
        let names: HashMap<AugmentId, String> = [
            (3116, "Take 'em Down\nPune-i la pământ"),
            (1020, "Dawnbringer's Resolve\nVoința întrupării Luminii"),
            (1336, "Upgrade Infinity Edge\nÎmbunătățire: Sabia infinitului"),
            (1373, "Shrink Engine\nMotor de micșorare"),
        ]
        .into_iter()
        .map(|(id, n)| (id, n.to_string()))
        .collect();
        let lines: Vec<String> =
            ["Pune-i la pämånt", "Daune", "Vointa intrupärii Luminii", "Rezistentä", "imbunätätire: Sabia", "infinitului", "Daune"]
                .map(String::from)
                .to_vec();
        assert_eq!(match_augments(&lines, &names), [3116, 1020, 1336]);
        assert_eq!(match_augments(&["Shrink Engine".to_string(), "Motor de micsorare".to_string()], &names), [1373]);
    }

    #[test]
    fn names_inside_longer_lines_are_not_titles() {
        let names: HashMap<AugmentId, String> =
            [(1, "Vulnerability"), (2, "POWER")].into_iter().map(|(id, n)| (id, n.to_string())).collect();
        let lines: Vec<String> =
            ["500", "& 800", "Zhonya's Hourglass", "C) Invulnerability", "105 Ability Power"].map(String::from).to_vec();
        assert!(match_augments(&lines, &names).is_empty());
        assert!(run(&["Back to Basics: your abilities deal more"]).is_empty());
    }

    #[test]
    fn longer_name_wins_overlap() {
        assert_eq!(run(&["Courage of the Colossus", "Colossus"]), [3, 4]);
        assert_eq!(run(&["Courage of the Colossus"]), [3]);
    }

    #[test]
    fn rejects_unrelated() {
        assert!(
            run(&[
                "Reroll",
                "Select an augment",
                "PRISMATIC",
                "Gold",
                "0:42",
                ""
            ])
            .is_empty()
        );
        assert!(run(&["Goliad", "Firebird", "Erasure"]).is_empty());
        assert!(run(&[]).is_empty());
    }

    #[test]
    fn dedupes_and_caps() {
        assert_eq!(run(&["Goliath", "Goliath", "Erosion"]), [10, 8]);
        assert_eq!(
            run(&["Goliath Erosion Firebrand Ice Cold Mad Scientist"]).len(),
            4
        );
    }
}
