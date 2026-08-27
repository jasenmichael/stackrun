use super::discover::{is_js_ts_path, resolve_config_file};
use super::dotenv::load_dotenv;
use super::merge::{apply_env_overlay, defu, take_extends};
use super::parse::{parse_file, parse_json_overlay};
use super::rc::parse_rc_file;
use super::types::StackrunConfig;
use crate::bridge;
use crate::cli::Cli;
use crate::error::Error;
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
}

impl LoadOptions {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            config_file: cli.config_path(),
            json_overlay: cli.json.clone(),
            command: cli.command.clone(),
            tunnel_flag: cli.tunnel,
        }
    }

    pub fn for_cwd(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            config_file: DEFAULT_CONFIG_FILE.to_string(),
            json_overlay: None,
            command: None,
            tunnel_flag: false,
        }
    }
}

#[derive(Debug)]
pub struct LoadedConfig {
    pub config: StackrunConfig,
    pub config_file: Option<PathBuf>,
    pub merged: Value,
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
        layers.push(parse_json_overlay(json)?);
    }

    if let Some(path) = &resolved {
        let mut main = load_layer(&options.cwd, path)?;
        let extends = take_extends(&mut main)?;
        let mut extended = Vec::new();
        for source in extends {
            let ext_path = resolve_extends_path(path, &source);
            let layer = load_layer(&options.cwd, &ext_path)?;
            extended.push(layer);
        }
        // c12: defu(main, ...layers) — main wins, extends are fallbacks
        let mut combined = main;
        for layer in extended {
            combined = defu(combined, layer);
        }
        layers.push(combined);
    }

    let rc_path = options.cwd.join(".stackrc");
    if rc_path.is_file() {
        let mut rc = parse_rc_file(&rc_path)?;
        rc = apply_env_overlay(rc, env::var("NODE_ENV").ok().as_deref());
        layers.push(rc);
    }

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

    Ok(LoadedConfig {
        config,
        config_file: resolved,
        merged,
    })
}

fn load_layer(cwd: &Path, path: &Path) -> Result<Value, Error> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if is_js_ts_path(&abs) {
        let mut value = bridge::load_js_ts(&abs)?;
        value = apply_env_overlay(value, env::var("NODE_ENV").ok().as_deref());
        return Ok(value);
    }
    let mut value = parse_file(&abs)?;
    value = apply_env_overlay(value, env::var("NODE_ENV").ok().as_deref());
    Ok(value)
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
        config.tunnel_enabled = Some(true);
    }
}

fn apply_cli_overrides(config: &mut StackrunConfig, options: &LoadOptions) {
    if options.tunnel_flag {
        config.tunnel_enabled = Some(true);
    }
    if let Some(command) = &options.command {
        config.commands = Some(vec![super::types::CommandEntry::Shell(command.clone())]);
    }
}

/// Defaults applied at run time. Explicit `handleInput: false` is honored.
pub fn apply_defaults(config: &mut StackrunConfig) {
    let mut opts = config.concurrently_options.clone().unwrap_or_default();
    if opts.kill_others.is_none() {
        opts.kill_others = Some(super::types::KillOthers::One("failure".into()));
    }
    if opts.handle_input.is_none() {
        opts.handle_input = Some(true);
    }
    if opts.prefix_colors.is_none() {
        opts.prefix_colors = Some(super::types::PrefixColors::Named("auto".into()));
    }
    if opts.prefix_length.is_none() {
        opts.prefix_length = Some(10);
    }
    config.concurrently_options = Some(opts);

    if config.tunnel_enabled.is_none() {
        config.tunnel_enabled = Some(false);
    }
    if config.before_commands.is_none() {
        config.before_commands = Some(Vec::new());
    }
    if config.after_commands.is_none() {
        config.after_commands = Some(Vec::new());
    }
    if config.commands.is_none() {
        config.commands = Some(Vec::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn yaml_file_and_tunnel_env() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("stack.config.yaml"),
            "commands:\n  - command: echo hi\n    name: hi\n",
        )
        .unwrap();
        env::set_var("TUNNEL", "true");
        let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
        env::remove_var("TUNNEL");
        assert!(loaded.config.tunnel_enabled());
        assert_eq!(loaded.config.runnable_commands()[0].command, "echo hi");
    }

    #[test]
    fn command_override_does_not_need_file() {
        let dir = tempdir().unwrap();
        let mut opts = LoadOptions::for_cwd(dir.path());
        opts.command = Some("python server.py".into());
        let loaded = load_config(opts).unwrap();
        assert_eq!(
            loaded.config.runnable_commands()[0].command,
            "python server.py"
        );
        assert!(loaded.config_file.is_none());
    }

    #[test]
    fn missing_file_errors() {
        let dir = tempdir().unwrap();
        let err = load_config(LoadOptions::for_cwd(dir.path())).unwrap_err();
        assert!(matches!(err, Error::ConfigNotFound { .. }));
    }
}
