use clap::Parser;

/// Run multiple services with optional Cloudflare tunneling.
#[derive(Debug, Parser)]
#[command(
    name = "stackrun",
    version,
    disable_version_flag = true,
    about = "Run multiple services with optional Cloudflare tunneling"
)]
pub struct Cli {
    /// Path to config file (extension optional). Default matches Node/c12: `stack.config`.
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,

    /// Input config as JSON (CLI overlay; highest data priority).
    #[arg(long = "json")]
    pub json: Option<String>,

    /// Enable tunneling.
    #[arg(short = 't', long = "tunnel")]
    pub tunnel: bool,

    /// Print version and exit (Node `-V` / `--version` prints the version string).
    #[arg(short = 'V', long = "version")]
    pub print_version: bool,

    /// Run a single shell command without a config file.
    ///
    /// New in the Rust CLI. Node stackrun has no `--command` flag.
    #[arg(long = "command")]
    pub command: Option<String>,

    /// Optional positional config path (`args._[0]` in the Node CLI).
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
}
