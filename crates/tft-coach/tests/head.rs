//! The classifier head in Rust gives head.py's top-1 and probability (f16 weights): on the shipped
//! `head.bin` with its parity cases when one exists, and always on a tiny head head.py wrote.

use tft_coach::Library;
use tft_coach::head::Head;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/tft/");

fn check(head: &str, cases: &str) {
    let bytes = std::fs::read(format!("{ROOT}{head}")).unwrap();
    let h = Head::from_bytes(&bytes).unwrap();
    let cases: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{ROOT}{cases}")).unwrap()).unwrap();
    let cases = cases.as_array().unwrap();
    assert!(cases.len() >= 100, "{} cases", cases.len());
    let lib = Library::default().with_head(&bytes);
    for c in cases {
        let x: Vec<f32> = c["input"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        let s = h.scores(&x);
        assert_eq!(s[0].0, c["top1"].as_str().unwrap());
        assert!((s[0].1 - c["score"].as_f64().unwrap() as f32).abs() <= 5e-3, "{} vs {}", s[0].1, c["score"]);
        assert_eq!(lib.classify_head(&x)[0], s[0]);
    }
}

#[test]
fn the_tiny_head_matches_head_py() {
    check("head_tiny.bin", "head_tiny_parity.json");
}

#[test]
fn the_shipped_head_matches_head_py_or_is_off() {
    let bytes = std::fs::read(format!("{ROOT}head.bin")).unwrap();
    if bytes.is_empty() {
        let lib = Library::default().with_head(&bytes);
        assert!(!lib.has_head() && lib.head_name(&[0.0; 768]).is_none() && lib.classify_head(&[0.0; 768]).is_empty());
        return;
    }
    check("head.bin", "head_parity.json");
}
