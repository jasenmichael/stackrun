use clap::{Parser, ValueEnum};

/// How to resolve `jiti` when loading a JS/TS config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum JitiMode {
    /// `node` + `import("jiti")` from the project directory only.
    #[default]
    Local,
    /// After a local miss, retry with `npx -p jiti@2 node ...`.
    Npx,
}

/// Run multiple services with optional Cloudflare tunneling.
#[derive(Debug, Parser)]
#[command(
    name = "stackrun",
    version,
    disable_version_flag = true,
    about = "Run multiple services with optional Cloudflare tunneling"
)]
pub struct Cli {
    /// Path to config file (extension optional). Default: `stack.config`.
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,

    /// Input config as JSON (CLI overlay; highest data priority).
    #[arg(long = "json")]
    pub json: Option<String>,

    /// Enable tunneling.
    #[arg(short = 't', long = "tunnel")]
    pub tunnel: bool,

    /// Print version and exit.
    #[arg(short = 'V', long = "version")]
    pub print_version: bool,

    /// Run a single shell command without a config file.
    #[arg(long = "command")]
    pub command: Option<String>,

    /// Load config and print effective options as JSON. Does not spawn
    /// processes, tunnels, before hooks, or after hooks.
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Resolve jiti for JS/TS configs. `local` uses the project only.
    /// `npx` retries via `npx -p jiti@2` (first run may need network).
    #[arg(long = "jiti", env = "STACKRUN_JITI", value_enum, default_value_t = JitiMode::Local)]
    pub jiti: JitiMode,

    /// Optional positional config path.
    #[arg(value_name = "CONFIG")]
    pub config_positional: Option<String>,
}

impl Cli {
    /// Resolve config path: `-c` / `--config` / positional / `stack.config`.
    pub fn config_path(&self) -> String {
        self.config
            .clone()
            .or_else(|| self.config_positional.clone())
            .unwrap_or_else(|| "stack.config".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn default_config_path_matches_node() {
        let cli = Cli::parse_from(["stackrun"]);
        assert_eq!(cli.config_path(), "stack.config");
    }

    #[test]
    fn positional_config_used_when_flag_absent() {
        let cli = Cli::parse_from(["stackrun", "custom.yaml"]);
        assert_eq!(cli.config_path(), "custom.yaml");
    }

    #[test]
    fn short_c_wins_over_positional() {
        let cli = Cli::parse_from(["stackrun", "-c", "a.yaml", "b.yaml"]);
        assert_eq!(cli.config_path(), "a.yaml");
    }

    #[test]
    fn dry_run_flag_parses() {
        let cli = Cli::parse_from(["stackrun", "--dry-run", "--command", "echo x"]);
        assert!(cli.dry_run);
        assert_eq!(cli.command.as_deref(), Some("echo x"));
    }

    #[test]
    fn jiti_defaults_to_local() {
        std::env::remove_var("STACKRUN_JITI");
        let cli = Cli::parse_from(["stackrun"]);
        assert_eq!(cli.jiti, JitiMode::Local);
    }

    #[test]
    fn jiti_npx_flag_parses() {
        let cli = Cli::parse_from(["stackrun", "--jiti", "npx"]);
        assert_eq!(cli.jiti, JitiMode::Npx);
    }
}
