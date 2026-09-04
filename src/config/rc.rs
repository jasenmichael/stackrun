use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// Parse an rc9-style file (KEY=VALUE lines, dotted keys unflattened).
///
/// This is what c12 uses for `.stackrc` via `rc9.read`. It is **not** JSON.
pub fn parse_rc_file(path: &Path) -> Result<Value, crate::error::Error> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let contents = fs::read_to_string(path)?;
    Ok(parse_rc(&contents))
}

pub fn parse_rc(contents: &str) -> Value {
    let mut root = Value::Object(Map::new());
    for line in contents.split(['\n', '\r']) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, raw)) = split_key_val(line) else {
            continue;
        };
        if key.is_empty() || key == "__proto__" || key == "constructor" {
            continue;
        }
        let value = destr(raw.trim());
        if let Some(array_key) = key.strip_suffix("[]") {
            push_path(&mut root, array_key, value, true);
        } else {
            push_path(&mut root, key, value, false);
        }
    }
    root
}

fn split_key_val(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some((key, &line[eq + 1..]))
}

/// Approximate unjs/destr: JSON scalars, else raw string.
fn destr(raw: &str) -> Value {
    if raw.is_empty() {
        return Value::String(String::new());
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return v;
    }
    if let Ok(v) = json5::from_str::<Value>(raw) {
        return v;
    }
    Value::String(raw.to_string())
}

fn push_path(root: &mut Value, path: &str, value: Value, as_array: bool) {
    let parts: Vec<&str> = path.split('.').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return;
    }
    let mut cur = root;
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        if last {
            let obj = object_mut(cur);
            if as_array {
                match obj.remove(*part) {
                    Some(Value::Array(mut arr)) => {
                        arr.push(value);
                        obj.insert((*part).to_string(), Value::Array(arr));
                    }
                    Some(other) => {
                        obj.insert((*part).to_string(), Value::Array(vec![other, value]));
                    }
                    None => {
                        obj.insert((*part).to_string(), Value::Array(vec![value]));
                    }
                }
            } else {
                obj.insert((*part).to_string(), value);
            }
            return;
        }
        let obj = object_mut(cur);
        if !obj.get(*part).is_some_and(Value::is_object) {
            obj.insert((*part).to_string(), Value::Object(Map::new()));
        }
        cur = obj.get_mut(*part).unwrap();
    }
}

fn object_mut(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn unflattens_and_parses_json_values() {
        let v = parse_rc("tunnel.resource=dev\n");
        assert_eq!(
            v,
            json!({
                "tunnel": { "resource": "dev" }
            })
        );
    }

    #[test]
    fn array_suffix() {
        let v = parse_rc("before[]=echo a\nbefore[]=echo b\n");
        assert_eq!(v["before"], json!(["echo a", "echo b"]));
    }
}
