use serde_json::{Map, Value};
use std::env;
use std::fs;
use std::path::Path;

/// Load `.env` from `cwd` into the process environment.
///
/// Matches c12 `dotenv: true`:
/// - file name `.env`
/// - interpolate `${VAR}` / `$VAR`
/// - do not override variables already set in the real environment
/// - skip keys that start with `_`
pub fn load_dotenv(cwd: &Path) -> Result<(), crate::error::Error> {
    let path = cwd.join(".env");
    if !path.is_file() {
        return Ok(());
    }
    let contents = fs::read_to_string(&path)?;
    let parsed = parse_env_file(&contents);
    let interpolated = interpolate(&parsed);
    for (key, value) in interpolated {
        if key.starts_with('_') {
            continue;
        }
        if env::var_os(&key).is_none() {
            env::set_var(key, value);
        }
    }
    Ok(())
}

pub fn parse_env_file(contents: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = unquote(value.trim());
        out.push((key.to_string(), value));
    }
    out
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

/// c12-style interpolation of `$VAR` and `${VAR}`. Existing env wins over file values.
pub fn interpolate(pairs: &[(String, String)]) -> Vec<(String, String)> {
    let mut map: Vec<(String, String)> = pairs.to_vec();
    let keys: Vec<String> = map.iter().map(|(k, _)| k.clone()).collect();
    for key in keys {
        let value = map
            .iter()
            .find(|(k, _)| k == &key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let rendered = interpolate_value(&value, &map, &mut Vec::new());
        if let Some(slot) = map.iter_mut().find(|(k, _)| k == &key) {
            slot.1 = rendered;
        }
    }
    map
}

fn interpolate_value(value: &str, file: &[(String, String)], parents: &mut Vec<String>) -> String {
    let mut out = String::new();
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
            out.push('$');
            i += 2;
            continue;
        }
        if chars[i] == '$' {
            let (name, consumed) = parse_var_name(&chars[i + 1..]);
            if let Some(name) = name {
                if parents.contains(&name) {
                    i += 1 + consumed;
                    continue;
                }
                parents.push(name.clone());
                let raw = lookup(&name, file);
                let nested = interpolate_value(&raw, file, parents);
                parents.pop();
                out.push_str(&nested);
                i += 1 + consumed;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn parse_var_name(rest: &[char]) -> (Option<String>, usize) {
    if rest.first() == Some(&'{') {
        let mut name = String::new();
        let mut i = 1;
        while i < rest.len() && rest[i] != '}' {
            name.push(rest[i]);
            i += 1;
        }
        if i < rest.len() && rest[i] == '}' && !name.is_empty() {
            return (Some(name), i + 1);
        }
        return (None, 0);
    }
    let mut name = String::new();
    let mut i = 0;
    while i < rest.len() && (rest[i].is_ascii_alphanumeric() || rest[i] == '_') {
        name.push(rest[i]);
        i += 1;
    }
    if name.is_empty() {
        (None, 0)
    } else {
        (Some(name), i)
    }
}

fn lookup(name: &str, file: &[(String, String)]) -> String {
    if let Ok(v) = env::var(name) {
        return v;
    }
    file.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// Helper for tests: parse `.env` content to a JSON object of interpolated values
/// without writing process env.
pub fn parse_env_to_value(contents: &str) -> Value {
    let pairs = interpolate(&parse_env_file(contents));
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k, Value::String(v));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn interpolates_braces() {
        let v = parse_env_to_value("BASE=/tmp\nDIR=${BASE}/app\n");
        assert_eq!(v, json!({"BASE": "/tmp", "DIR": "/tmp/app"}));
    }
}
