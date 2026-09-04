use crate::error::Error;
use serde_json::Value;
use std::path::Path;

/// Parse a native config file into JSON (canonical merge representation).
pub fn parse_file(path: &Path) -> Result<Value, Error> {
    let contents = std::fs::read_to_string(path)?;
    parse_str(path, &contents)
}

pub fn parse_str(path: &Path, contents: &str) -> Result<Value, Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "json" => parse_json(path, contents),
        "jsonc" => parse_json5(path, contents, "jsonc"),
        "json5" => parse_json5(path, contents, "json5"),
        "yaml" | "yml" => parse_yaml(path, contents),
        "toml" => parse_toml(path, contents),
        other => Err(Error::Parse {
            path: path.display().to_string(),
            format: "unknown",
            message: format!("unsupported config extension .{other}"),
        }),
    }
}

fn parse_json(path: &Path, contents: &str) -> Result<Value, Error> {
    serde_json::from_str(contents).map_err(|err| Error::Parse {
        path: path.display().to_string(),
        format: "json",
        message: err.to_string(),
    })
}

fn parse_json5(path: &Path, contents: &str, format: &'static str) -> Result<Value, Error> {
    json5::from_str(contents).map_err(|err| Error::Parse {
        path: path.display().to_string(),
        format,
        message: err.to_string(),
    })
}

fn parse_yaml(path: &Path, contents: &str) -> Result<Value, Error> {
    let yaml: serde_yml::Value = serde_yml::from_str(contents).map_err(|err| Error::Parse {
        path: path.display().to_string(),
        format: "yaml",
        message: err.to_string(),
    })?;
    serde_json::to_value(yaml).map_err(|err| Error::Parse {
        path: path.display().to_string(),
        format: "yaml",
        message: err.to_string(),
    })
}

fn parse_toml(path: &Path, contents: &str) -> Result<Value, Error> {
    let toml: toml::Value = toml::from_str(contents).map_err(|err| Error::Parse {
        path: path.display().to_string(),
        format: "toml",
        message: err.to_string(),
    })?;
    serde_json::to_value(toml).map_err(|err| Error::Parse {
        path: path.display().to_string(),
        format: "toml",
        message: err.to_string(),
    })
}

pub fn parse_json_overlay(json: &str) -> Result<Value, Error> {
    serde_json::from_str(json).map_err(|err| Error::Parse {
        path: "<--json>".to_string(),
        format: "json",
        message: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn jsonc_allows_comments_and_trailing_commas() {
        let v = parse_str(
            Path::new("x.jsonc"),
            r#"{
              // comment
              "tunnel": true,
              "commands": [{ "run": "echo hi", }],
            }"#,
        )
        .unwrap();
        assert_eq!(v["tunnel"], true);
        assert_eq!(v["commands"][0]["run"], "echo hi");
    }

    #[test]
    fn json_commands() {
        let v = parse_str(Path::new("x.json"), r#"{"commands":[{"run":"echo json"}]}"#).unwrap();
        assert_eq!(v["commands"][0]["run"], "echo json");
    }

    #[test]
    fn json5_commands() {
        let v = parse_str(
            Path::new("x.json5"),
            "{ commands: [{ run: 'echo json5' }] }",
        )
        .unwrap();
        assert_eq!(v["commands"][0]["run"], "echo json5");
    }

    #[test]
    fn yaml_commands() {
        let v = parse_str(
            Path::new("x.yaml"),
            "commands:\n  - name: api\n    run: python server.py\n",
        )
        .unwrap();
        assert_eq!(v["commands"][0]["name"], "api");
    }

    #[test]
    fn toml_commands() {
        let v = parse_str(
            Path::new("x.toml"),
            r#"
[[commands]]
name = "web"
run = "npm run dev"
"#,
        )
        .unwrap();
        assert_eq!(v["commands"][0]["name"], "web");
    }

    #[test]
    fn unknown_extension_errors() {
        let err = parse_str(&PathBuf::from("x.ini"), "foo=1").unwrap_err();
        assert!(matches!(
            err,
            Error::Parse {
                format: "unknown",
                ..
            }
        ));
    }
}
