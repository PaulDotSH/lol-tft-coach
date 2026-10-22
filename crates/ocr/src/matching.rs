//! Fuzzy matching of OCR text against augment names.

use model::AugmentId;
use std::collections::HashMap;

const MAX_OFFERS: usize = 4;

/// Lowercase alphanumeric words with common OCR confusions folded.
/// Applied to both OCR text and names, so folding is symmetric.
fn words(s: &str) -> Vec<String> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\'' | '\u{2019}' | '`' => {}
            '0' => out.push('o'),
            '1' | '|' => out.push('l'),
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

struct Hit {
    id: AugmentId,
    start: usize,
    end: usize,
    score: i64,
}

/// Augments whose names appear in `lines` (fuzzy), in screen order, deduped.
///
/// OCR words are concatenated (spaces dropped, so split/merged words don't
/// matter) over windows of the name's word count ±1, which may span one line
/// break (wrapped names). Overlapping hits are resolved in favour of the hit
/// explaining more characters.
pub fn match_augments(lines: &[String], names: &HashMap<AugmentId, String>) -> Vec<AugmentId> {
    let ocr: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .flat_map(|(i, l)| words(l).into_iter().map(move |w| (i, w)))
        .collect();
    let targets: Vec<Target> = names
        .iter()
        .filter_map(|(&id, name)| {
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
                    hits.push(Hit {
                        id: t.id,
                        start,
                        end,
                        score,
                    });
                }
            }
        }
    }

    hits.sort_by_key(|h| (std::cmp::Reverse(h.score), h.start));
    let mut taken: Vec<&Hit> = Vec::new();
    for h in &hits {
        if !taken
            .iter()
            .any(|t| t.id == h.id || (h.start < t.end && t.start < h.end))
        {
            taken.push(h);
        }
    }
    taken.sort_by_key(|h| h.start);
    taken.iter().take(MAX_OFFERS).map(|h| h.id).collect()
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
        assert_eq!(
            run(&[
                "Select an augment",
                "Back to Basics: your abilities deal more",
                "Reroll"
            ]),
            [9]
        );
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
