use serde_json::{Map, Value};

/// defu-like merge: `preferred` wins over `fallback`.
///
/// Objects deep-merge. Arrays concatenate with `preferred` first, then unique
/// `fallback` items (JSON equality). Other values: `preferred` wins, including `null`.
pub fn defu(preferred: Value, fallback: Value) -> Value {
    match (preferred, fallback) {
        (Value::Object(mut a), Value::Object(b)) => {
            for (key, fb) in b {
                match a.remove(&key) {
                    Some(pref) => {
                        a.insert(key, defu(pref, fb));
                    }
                    None => {
                        a.insert(key, fb);
                    }
                }
            }
            Value::Object(a)
        }
        (Value::Array(pref), Value::Array(fb)) => {
            let mut out = pref;
            for item in fb {
                if !out.contains(&item) {
                    out.push(item);
                }
            }
            Value::Array(out)
        }
        (pref, _) => pref,
    }
}

/// Apply c12 environment-specific overlays using `NODE_ENV` (`envName`).
///
/// `config = defu({ ...$<env>, ...$env[env] }, config)`
pub fn apply_env_overlay(mut config: Value, env_name: Option<&str>) -> Value {
    let Some(env_name) = env_name.filter(|s| !s.is_empty()) else {
        return config;
    };
    let mut overlay = Value::Object(Map::new());
    if let Some(obj) = config.as_object() {
        if let Some(named) = obj.get(&format!("${env_name}")) {
            overlay = defu(named.clone(), overlay);
        }
        if let Some(env_map) = obj.get("$env").and_then(|v| v.as_object()) {
            if let Some(named) = env_map.get(env_name) {
                overlay = defu(named.clone(), overlay);
            }
        }
    }
    if overlay.as_object().is_some_and(|o| !o.is_empty()) {
        config = defu(overlay, config);
    }
    config
}

/// Pull `extends` (string or array of strings) off a layer. Remote URIs error.
pub fn take_extends(config: &mut Value) -> Result<Vec<String>, crate::error::Error> {
    let Some(obj) = config.as_object_mut() else {
        return Ok(Vec::new());
    };
    let Some(extends) = obj.remove("extends") else {
        return Ok(Vec::new());
    };
    let sources: Vec<String> = match extends {
        Value::String(s) if !s.is_empty() => vec![s],
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    for source in &sources {
        if is_remote_extends(source) {
            return Err(crate::error::Error::RemoteExtends {
                uri: source.clone(),
            });
        }
    }
    Ok(sources)
}

pub fn is_remote_extends(source: &str) -> bool {
    let prefixes = [
        "gh:",
        "github:",
        "gitlab:",
        "bitbucket:",
        "https://",
        "http://",
        "npm:",
    ];
    prefixes.iter().any(|p| source.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn objects_deep_merge_preferred_wins() {
        let a = json!({"tunnel": true, "process": {"prefixLength": 10}});
        let b = json!({"tunnel": false, "process": {"prefixLength": 8, "handleInput": false}});
        let m = defu(a, b);
        assert_eq!(m["tunnel"], true);
        assert_eq!(m["process"]["prefixLength"], 10);
        assert_eq!(m["process"]["handleInput"], false);
    }

    #[test]
    fn arrays_concat_unique() {
        let a = json!(["a", "b"]);
        let b = json!(["b", "c"]);
        assert_eq!(defu(a, b), json!(["a", "b", "c"]));
    }

    #[test]
    fn env_overlay_development() {
        let raw = json!({
            "tunnel": false,
            "$development": { "tunnel": true },
            "$env": { "staging": { "tunnel": true } }
        });
        let out = apply_env_overlay(raw, Some("development"));
        assert_eq!(out["tunnel"], true);
    }
}
