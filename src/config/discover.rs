use std::path::{Path, PathBuf};

/// c12 `SUPPORTED_EXTENSIONS` order. JS/TS come first; if both a `.ts` and `.yaml`
/// exist for the same base name, JS/TS wins and requires the Jiti bridge.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    ".js", ".ts", ".mjs", ".cjs", ".mts", ".cts", ".json", ".jsonc", ".json5", ".yaml", ".yml",
    ".toml",
];

pub const JS_TS_EXTENSIONS: &[&str] = &[".js", ".ts", ".mjs", ".cjs", ".mts", ".cts"];

pub fn is_js_ts_path(path: &Path) -> bool {
    extension_with_dot(path).is_some_and(|ext| JS_TS_EXTENSIONS.contains(&ext.as_str()))
}

pub fn is_native_path(path: &Path) -> bool {
    matches!(
        extension_with_dot(path).as_deref(),
        Some(".json" | ".jsonc" | ".json5" | ".yaml" | ".yml" | ".toml")
    )
}

fn extension_with_dot(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
}

/// True when `configFile` has no *loader* extension (c12 still treats `stack.config` as a base name).
pub fn is_bare_config_name(config_file: &str) -> bool {
    let path = Path::new(config_file);
    match extension_with_dot(path).as_deref() {
        None => true,
        Some(ext) => !SUPPORTED_EXTENSIONS.contains(&ext),
    }
}

/// Resolve a config path the way c12 `resolveConfig` does for `name: "stack"`.
///
/// Search order for a bare name such as `stack.config`:
/// 1. `{cwd}/{name}{ext}`
/// 2. `{cwd}/.config/{name-without-.config}{ext}`
/// 3. `{cwd}/.config/{name}{ext}`
///
/// First existing file in `SUPPORTED_EXTENSIONS` order wins.
pub fn resolve_config_file(cwd: &Path, config_file: &str) -> Option<PathBuf> {
    let given = Path::new(config_file);
    let abs = if given.is_absolute() {
        given.to_path_buf()
    } else {
        cwd.join(given)
    };

    if !is_bare_config_name(config_file) {
        return abs.is_file().then_some(abs);
    }

    if abs.is_file() {
        return Some(abs);
    }

    let name = Path::new(config_file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(config_file);
    let without_config_suffix = name.strip_suffix(".config").unwrap_or(name);

    let bases = [
        cwd.join(name),
        cwd.join(".config").join(without_config_suffix),
        cwd.join(".config").join(name),
    ];

    for base in &bases {
        for ext in SUPPORTED_EXTENSIONS {
            let candidate = PathBuf::from(format!("{}{ext}", base.display()));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn stack_config_is_bare() {
        assert!(is_bare_config_name("stack.config"));
        assert!(is_bare_config_name("custom"));
        assert!(!is_bare_config_name("stack.config.yaml"));
        assert!(!is_bare_config_name("app.json"));
    }

    #[test]
    fn discovers_yaml_and_config_dir() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("stack.config.yaml"), "commands: []\n").unwrap();
        let found = resolve_config_file(dir.path(), "stack.config").unwrap();
        assert_eq!(found.file_name().unwrap(), "stack.config.yaml");

        let dir2 = tempdir().unwrap();
        fs::create_dir(dir2.path().join(".config")).unwrap();
        fs::write(dir2.path().join(".config/stack.toml"), "commands = []\n").unwrap();
        let found = resolve_config_file(dir2.path(), "stack.config").unwrap();
        assert!(found.ends_with(".config/stack.toml"));
    }

    #[test]
    fn js_wins_over_yaml_when_both_exist() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("stack.config.ts"), "export default {}\n").unwrap();
        fs::write(dir.path().join("stack.config.yaml"), "commands: []\n").unwrap();
        let found = resolve_config_file(dir.path(), "stack.config").unwrap();
        assert_eq!(found.extension().unwrap(), "ts");
    }

    #[test]
    fn explicit_yaml_ignores_sibling_ts() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("stack.config.ts"), "export default {}\n").unwrap();
        fs::write(dir.path().join("stack.config.yaml"), "commands: []\n").unwrap();
        let found = resolve_config_file(dir.path(), "stack.config.yaml").unwrap();
        assert_eq!(found.extension().unwrap(), "yaml");
    }
}
