//! Minimal decoder for Qwik's serialized state (`<script type="qwik/json">`).
//!
//! The blob is `{"objs": [...], ...}`. Inside objects and arrays every value is a
//! base-36 string index into `objs`; top-level `objs` entries that are not
//! containers are the primitive values themselves (strings may carry a leading
//! control char for special types, e.g. `\u{1}` = undefined).

use serde_json::{Map, Value};

pub struct Qwik {
    objs: Vec<Value>,
}

/// Extract and parse the first Qwik state blob from an HTML page.
pub fn parse(html: &str) -> Option<Qwik> {
    let tag = html.find(r#"<script type="qwik/json">"#)?;
    let start = tag + html[tag..].find('>')? + 1;
    let end = start + html[start..].find("</script>")?;
    let mut root: Value = serde_json::from_str(&html[start..end]).ok()?;
    match root.get_mut("objs")?.take() {
        Value::Array(objs) => Some(Qwik { objs }),
        _ => None,
    }
}

impl Qwik {
    /// Fully resolve the value referenced by `r` (cycles cut at `depth`).
    pub fn resolve(&self, r: &Value, depth: u8) -> Value {
        let Some(obj) = r.as_str().and_then(|s| self.get(s)) else { return r.clone() };
        self.expand(obj, depth)
    }

    fn get(&self, r: &str) -> Option<&Value> {
        self.objs.get(usize::from_str_radix(r, 36).ok()?)
    }

    fn expand(&self, obj: &Value, depth: u8) -> Value {
        match obj {
            Value::Object(m) if depth > 0 => {
                Value::Object(m.iter().map(|(k, v)| (k.clone(), self.resolve(v, depth - 1))).collect())
            }
            Value::Array(a) if depth > 0 => Value::Array(a.iter().map(|v| self.resolve(v, depth - 1)).collect()),
            Value::Object(_) | Value::Array(_) => Value::Null,
            Value::String(s) if s.starts_with(|c: char| c.is_control()) => Value::Null,
            v => v.clone(),
        }
    }

    /// Resolve (to `depth`) each top-level object having all `keys`, and return
    /// the first one `f` accepts.
    pub fn find_map<T>(&self, depth: u8, keys: &[&str], f: impl FnMut(Value) -> Option<T>) -> Option<T> {
        let has_keys = |m: &Map<String, Value>| keys.iter().all(|k| m.contains_key(*k));
        self.objs.iter().filter(|o| o.as_object().is_some_and(has_keys)).map(|o| self.expand(o, depth)).find_map(f)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_refs() {
        let html = r#"<html><script type="qwik/json">{"objs":[{"a":"1","b":"2"},5.5,["3","4"],"x","\u0001",{"a":"3"}]}</script>"#;
        let q = super::parse(html).unwrap();
        let v = q.find_map(4, &["a", "b"], Some).unwrap();
        assert_eq!(v, serde_json::json!({"a": 5.5, "b": ["x", null]}));
        let s = q.find_map(4, &["a"], |v| v["a"].as_str().map(str::to_owned)).unwrap();
        assert_eq!(s, "x");
    }
}
