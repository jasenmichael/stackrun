use super::discover::{is_js_ts_path, resolve_config_file};
use super::dotenv::load_dotenv;
use super::merge::{apply_env_overlay, defu, take_extends};
use super::parse::{parse_file, parse_json_overlay};
use super::rc::parse_rc_file;
use super::types::StackrunConfig;
use crate::bridge;
use crate::cli::{Cli, JitiMode};
use crate::error::Error;
use serde::Serialize;
use serde_json::{json, Value};
use std::env;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_FILE: &str = "stack.config";

#[derive(Debug)]
pub struct LoadOptions {
    pub cwd: PathBuf,
    pub config_file: String,
    pub json_overlay: Option<String>,
    pub command: Option<String>,
    pub tunnel_flag: bool,
    pub jiti: JitiMode,
}

impl LoadOptions {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            config_file: cli.config_path(),
            json_overlay: cli.json.clone(),
            command: cli.command.clone(),
            tunnel_flag: cli.tunnel,
            jiti: cli.jiti,
        }
    }

    pub fn for_cwd(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            config_file: DEFAULT_CONFIG_FILE.to_string(),
            json_overlay: None,
            command: None,
            tunnel_flag: false,
            jiti: JitiMode::Local,
        }
    }
}

#[derive(Debug)]
pub struct LoadedConfig {
    pub config: StackrunConfig,
    pub config_file: Option<PathBuf>,
    pub merged: Value,
}

/// JSON envelope printed by `--dry-run`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunReport {
    /// Resolved config file path, if a file was used.
    pub config_file: Option<PathBuf>,
    /// Effective config after file load, RC, extends, env overlays, CLI flags,
    /// and defaults.
    pub config: StackrunConfig,
}

pub fn load_config(options: LoadOptions) -> Result<LoadedConfig, Error> {
    load_dotenv(&options.cwd)?;

    let resolved = resolve_config_file(&options.cwd, &options.config_file);
    let file_required = options.command.is_none() && options.json_overlay.is_none();

    if file_required && resolved.is_none() {
        return Err(Error::ConfigNotFound {
            path: options.config_file.clone(),
        });
    }

    let mut layers: Vec<Value> = Vec::new();

    if let Some(json) = &options.json_overlay {
        layers.push(normalize_legacy_keys(parse_json_overlay(json)?));
    }

    // SPEC: main > RC > extends (defu: earlier layer wins).
    let mut extends_layers = Vec::new();
    if let Some(path) = &resolved {
        let mut main = normalize_legacy_keys(load_layer(&options.cwd, path, options.jiti)?);
        let extends = take_extends(&mut main)?;
        for source in extends {
            let ext_path = resolve_extends_path(path, &source);
            extends_layers.push(normalize_legacy_keys(load_layer(
                &options.cwd,
                &ext_path,
                options.jiti,
            )?));
        }
        layers.push(main);
    }

    let rc_path = options.cwd.join(".stackrc");
    if rc_path.is_file() {
        let mut rc = parse_rc_file(&rc_path)?;
        rc = apply_env_overlay(rc, env::var("NODE_ENV").ok().as_deref());
        layers.push(normalize_legacy_keys(rc));
    }

    layers.extend(extends_layers);

    let merged = if layers.is_empty() {
        json!({})
    } else {
        let mut acc = layers.remove(0);
        for layer in layers {
            acc = defu(acc, layer);
        }
        acc
    };

    let mut config: StackrunConfig =
        serde_json::from_value(merged.clone()).map_err(|err| Error::Parse {
            path: resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| options.config_file.clone()),
            format: "stackrun",
            message: err.to_string(),
        })?;

    apply_env_var_overrides(&mut config);
    apply_cli_overrides(&mut config, &options);
    apply_defaults(&mut config);

    Ok(LoadedConfig {
        config,
        config_file: resolved,
        merged,
    })
}

fn load_layer(cwd: &Path, path: &Path, jiti: JitiMode) -> Result<Value, Error> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if is_js_ts_path(&abs) {
        let mut value = bridge::load_js_ts(&abs, cwd, jiti)?;
        value = apply_env_overlay(value, env::var("NODE_ENV").ok().as_deref());
        return Ok(value);
    }
    let mut value = parse_file(&abs)?;
    value = apply_env_overlay(value, env::var("NODE_ENV").ok().as_deref());
    Ok(value)
}

/// Rewrite one-cycle aliases onto the new keys so defu merges the same field
/// (`tunnelEnabled` vs `tunnel`, `beforeCommands` vs `before`, …).
fn normalize_legacy_keys(mut value: Value) -> Value {
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    rename_if_absent(obj, "beforeCommands", "before");
    rename_if_absent(obj, "afterCommands", "after");
    rename_if_absent(obj, "concurrentlyOptions", "process");
    if !obj.contains_key("tunnel") {
        if let Some(enabled) = obj.remove("tunnelEnabled") {
            obj.insert("tunnel".into(), enabled);
        }
    } else {
        obj.remove("tunnelEnabled");
    }
    value
}

fn rename_if_absent(obj: &mut serde_json::Map<String, Value>, old: &str, new: &str) {
    if !obj.contains_key(new) {
        if let Some(v) = obj.remove(old) {
            obj.insert(new.to_string(), v);
        }
    } else {
        obj.remove(old);
    }
}

fn resolve_extends_path(from_file: &Path, source: &str) -> PathBuf {
    let src = Path::new(source);
    if src.is_absolute() {
        return src.to_path_buf();
    }
    let dir = from_file.parent().unwrap_or_else(|| Path::new("."));
    dir.join(src)
}

fn apply_env_var_overrides(config: &mut StackrunConfig) {
    if env::var("TUNNEL").ok().as_deref() == Some("true") {
        config.force_tunnel = true;
    }
}

fn apply_cli_overrides(config: &mut StackrunConfig, options: &LoadOptions) {
    if options.tunnel_flag {
        config.force_tunnel = true;
    }
    if let Some(command) = &options.command {
        config.commands = Some(vec![super::types::CommandEntry::Shell(command.clone())]);
    }
}

/// No file-level secrets remain after dropping API tokens. Kept so `--dry-run`
/// stays a single load-then-print path.
pub fn redact_secrets(_config: &mut StackrunConfig) {}

/// Effective config for `--dry-run`: loaded config with secrets redacted.
/// Does not spawn processes or tunnels.
pub fn dry_run_report(loaded: &LoadedConfig) -> DryRunReport {
    let mut config = loaded.config.clone();
    redact_secrets(&mut config);
    DryRunReport {
        config_file: loaded.config_file.clone(),
        config,
    }
}

/// Pretty-print a [`DryRunReport`] as JSON (stable camelCase serde field names).
pub fn format_dry_run(loaded: &LoadedConfig) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&dry_run_report(loaded))
}

/// Defaults applied at load. Explicit `handleInput: false` is honored.
pub fn apply_defaults(config: &mut StackrunConfig) {
    let mut opts = config.process.clone().unwrap_or_default();
    if opts.kill_others.is_none() {
        opts.kill_others = Some(super::types::KillOthers::One("failure".into()));
    }
    if opts.handle_input.is_none() {
        opts.handle_input = Some(true);
    }
    if opts.colors.is_none() {
        opts.colors = Some(super::types::PrefixColors::Named("auto".into()));
    }
    if opts.prefix_length.is_none() {
        opts.prefix_length = Some(10);
    }
    config.process = Some(opts);

    use super::types::TunnelSetting;
    if config.force_tunnel {
        match &config.tunnel {
            Some(TunnelSetting::Defaults(_)) | Some(TunnelSetting::Flag(true)) => {}
            _ => config.tunnel = Some(TunnelSetting::Flag(true)),
        }
    } else if matches!(
        &config.tunnel,
        Some(TunnelSetting::Flag(false)) | Some(TunnelSetting::Flag(true))
    ) {
        // explicit off or on
    } else if config.has_any_tunnel_local() {
        if config.tunnel.is_none() {
            config.tunnel = Some(TunnelSetting::Defaults(Default::default()));
        }
    } else {
        config.tunnel = Some(TunnelSetting::Flag(false));
    }
    if config.before.is_none() {
        config.before = Some(Vec::new());
    }
    if config.after.is_none() {
        config.after = Some(Vec::new());
    }
    if config.commands.is_none() {
        config.commands = Some(Vec::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn yaml_file_and_tunnel_env() {
        let _g = env_lock();
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("stack.config.yaml"),
            "commands:\n  - run: echo hi\n    name: hi\n",
        )
        .unwrap();
        env::set_var("TUNNEL", "true");
        let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
        env::remove_var("TUNNEL");
        assert!(loaded.config.tunnel_enabled());
        assert_eq!(loaded.config.runnable_commands()[0].run, "echo hi");
    }

    #[test]
    fn command_override_does_not_need_file() {
        let dir = tempdir().unwrap();
        let mut opts = LoadOptions::for_cwd(dir.path());
        opts.command = Some("python server.py".into());
        let loaded = load_config(opts).unwrap();
        assert_eq!(loaded.config.runnable_commands()[0].run, "python server.py");
        assert!(loaded.config_file.is_none());
    }

    #[test]
    fn omitted_tunnel_follows_local() {
        let _g = env_lock();
        env::remove_var("TUNNEL");
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("stack.config.yaml"),
            "commands:\n  - run: echo hi\n    tunnel:\n      local: http://localhost:1\n",
        )
        .unwrap();
        let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
        assert!(loaded.config.tunnel_enabled());
        let report = dry_run_report(&loaded);
        assert!(report.config.tunnel_enabled());
        assert_eq!(
            loaded.config.process.as_ref().unwrap().kill_others,
            Some(crate::config::types::KillOthers::One("failure".into()))
        );
        assert_eq!(
            loaded.config.process.as_ref().unwrap().handle_input,
            Some(true)
        );
    }

    #[test]
    fn explicit_tunnel_false_wins_over_local() {
        let _g = env_lock();
        env::remove_var("TUNNEL");
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("stack.config.yaml"),
            "tunnel: false\ncommands:\n  - run: echo hi\n    tunnel:\n      local: http://localhost:1\n      public: https://x.example\n",
        )
        .unwrap();
        let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
        let report = dry_run_report(&loaded);
        assert!(!report.config.tunnel_enabled());
    }

    #[test]
    fn old_keys_still_load() {
        let _g = env_lock();
        env::remove_var("TUNNEL");
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("stack.config.yaml"),
            "tunnelEnabled: false\n\
             concurrentlyOptions:\n  handleInput: false\n  prefixColors: auto\n\
             beforeCommands:\n  - echo old-before\n\
             afterCommands:\n  - echo old-after\n\
             cfTunnelConfig:\n  removeExistingTunnel: true\n\
             commands:\n  - name: api\n    command: echo hi\n    prefixColor: green\n    url: http://localhost:1\n    tunnelUrl: https://x.example\n    tunnelEnv:\n      PUBLIC: x\n",
        )
        .unwrap();
        let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
        assert!(!loaded.config.tunnel_enabled());
        assert_eq!(
            loaded.config.before_commands(),
            &["echo old-before".to_string()]
        );
        assert_eq!(
            loaded.config.after_commands(),
            &["echo old-after".to_string()]
        );
        assert_eq!(
            loaded.config.process.as_ref().unwrap().handle_input,
            Some(false)
        );
        let cmd = &loaded.config.runnable_commands()[0];
        assert_eq!(cmd.run, "echo hi");
        assert_eq!(cmd.color.as_deref(), Some("green"));
        assert_eq!(cmd.tunnel_local(), Some("http://localhost:1"));
        assert_eq!(cmd.tunnel_public(), Some("https://x.example"));
        assert_eq!(
            cmd.tunnel
                .as_ref()
                .unwrap()
                .env
                .as_ref()
                .unwrap()
                .get("PUBLIC")
                .unwrap()
                .as_env_string(),
            Some("x".into())
        );
    }

    #[test]
    fn missing_file_errors() {
        let dir = tempdir().unwrap();
        let err = load_config(LoadOptions::for_cwd(dir.path())).unwrap_err();
        assert!(matches!(err, Error::ConfigNotFound { .. }));
    }

    #[test]
    fn dry_run_drops_legacy_token_and_keeps_path() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("stack.config.yaml"),
            "cfTunnelConfig:\n  cfToken: super-secret\ncommands:\n  - run: echo hi\n",
        )
        .unwrap();
        let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
        let report = dry_run_report(&loaded);
        let json = format_dry_run(&loaded).unwrap();
        assert!(!json.contains("super-secret"), "{json}");
        assert!(!json.contains("cfToken"), "{json}");
        assert!(!json.contains("cfTunnelConfig"), "{json}");
        assert!(report.config_file.is_some());
    }
}
