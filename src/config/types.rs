use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Canonical Stackrun configuration. Independent of c12 / Jiti / concurrently types.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StackrunConfig {
    pub concurrently_options: Option<ConcurrentlyOptions>,
    pub tunnel_enabled: Option<bool>,
    pub cf_tunnel_config: Option<CfTunnelConfig>,
    pub before_commands: Option<Vec<String>>,
    pub after_commands: Option<Vec<String>>,
    pub commands: Option<Vec<CommandEntry>>,
}

impl StackrunConfig {
    pub fn tunnel_enabled(&self) -> bool {
        self.tunnel_enabled.unwrap_or(false)
    }

    /// True when a command has both a non-empty `url` and `tunnelUrl`.
    pub fn has_tunnel_ingress(&self) -> bool {
        self.runnable_commands().iter().any(|c| {
            c.url.as_deref().is_some_and(|s| !s.is_empty())
                && c.tunnel_url.as_deref().is_some_and(|s| !s.is_empty())
        })
    }

    pub fn before_commands(&self) -> &[String] {
        self.before_commands.as_deref().unwrap_or(&[])
    }

    pub fn after_commands(&self) -> &[String] {
        self.after_commands.as_deref().unwrap_or(&[])
    }

    /// Commands that have a non-empty string `command` (Node filters the rest).
    pub fn runnable_commands(&self) -> Vec<CommandSpec> {
        self.commands
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(CommandEntry::to_spec)
            .filter(|c| !c.command.is_empty())
            .collect()
    }
}

/// concurrently options that Stackrun actually defaults / documents, plus passthrough extras.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConcurrentlyOptions {
    pub kill_others: Option<KillOthers>,
    pub handle_input: Option<bool>,
    pub prefix_colors: Option<PrefixColors>,
    pub prefix_length: Option<u32>,
    pub cwd: Option<String>,
    pub max_processes: Option<MaxProcesses>,
    pub raw: Option<bool>,
    pub restart_tries: Option<u32>,
    pub restart_delay: Option<u64>,
    pub success_condition: Option<String>,
    pub timings: Option<bool>,
    pub prefix: Option<String>,
    pub group: Option<bool>,
    pub kill_signal: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ConcurrentlyOptions {
    pub fn prefix_length_or_default(&self) -> usize {
        self.prefix_length.unwrap_or(10) as usize
    }

    /// Node uses `|| "failure"`, so a missing/falsy value becomes `"failure"`.
    pub fn kill_others_on_failure(&self) -> bool {
        match &self.kill_others {
            None => true,
            Some(KillOthers::One(s)) => s == "failure",
            Some(KillOthers::Many(items)) => items.iter().any(|s| s == "failure"),
        }
    }

    pub fn handle_input_or_default(&self) -> bool {
        self.handle_input.unwrap_or(true)
    }

    /// Color for a command at `index`. Explicit `prefixColor` wins. Otherwise
    /// cycle `prefixColors` / `"auto"`. `prefixColors: false` means no color.
    pub fn resolve_prefix_color(&self, explicit: Option<&str>, index: usize) -> Option<String> {
        if let Some(c) = explicit {
            if !c.is_empty() {
                return Some(c.to_string());
            }
        }
        match &self.prefix_colors {
            Some(PrefixColors::Flag(false)) => None,
            Some(PrefixColors::List(list)) if !list.is_empty() => {
                Some(list[index % list.len()].clone())
            }
            Some(PrefixColors::Named(s))
                if !s.eq_ignore_ascii_case("auto") && s != "true" && !s.is_empty() =>
            {
                Some(s.clone())
            }
            _ => {
                const AUTO: [&str; 7] =
                    ["cyan", "yellow", "green", "magenta", "blue", "red", "white"];
                Some(AUTO[index % AUTO.len()].to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KillOthers {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PrefixColors {
    Flag(bool),
    Named(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaxProcesses {
    Count(u32),
    Spec(String),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CfTunnelConfig {
    pub cf_token: Option<String>,
    pub tunnel_name: Option<String>,
    pub cloudflared_config_dir: Option<String>,
    pub remove_existing_tunnel: Option<bool>,
    pub remove_existing_dns: Option<bool>,
    pub command_options: Option<TunnelCommandOptions>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelCommandOptions {
    pub name: Option<String>,
    pub prefix_color: Option<String>,
    pub env: Option<BTreeMap<String, EnvValue>>,
    pub cwd: Option<String>,
    pub ipc: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandEntry {
    Shell(String),
    Full(CommandSpec),
}

impl CommandEntry {
    pub fn to_spec(&self) -> Option<CommandSpec> {
        match self {
            CommandEntry::Shell(command) if !command.is_empty() => Some(CommandSpec {
                command: command.clone(),
                ..CommandSpec::default()
            }),
            CommandEntry::Full(spec) if !spec.command.is_empty() => Some(spec.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandSpec {
    pub command: String,
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub env: Option<BTreeMap<String, EnvValue>>,
    pub prefix_color: Option<String>,
    pub ipc: Option<u32>,
    pub raw: Option<bool>,
    pub url: Option<String>,
    pub tunnel_url: Option<String>,
    pub tunnel_env: Option<BTreeMap<String, EnvValue>>,
}

impl CommandSpec {
    pub fn effective_env(&self, tunnel_enabled: bool) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        if let Some(env) = &self.env {
            for (k, v) in env {
                if let Some(s) = v.as_env_string() {
                    out.insert(k.clone(), s);
                }
            }
        }
        if tunnel_enabled {
            if let Some(env) = &self.tunnel_env {
                for (k, v) in env {
                    if let Some(s) = v.as_env_string() {
                        out.insert(k.clone(), s);
                    }
                }
            }
        }
        out
    }

    pub fn display_name(&self, prefix_length: usize) -> String {
        let raw = self.name.clone().unwrap_or_default();
        if prefix_length == 0 || raw.is_empty() {
            return raw;
        }
        raw.chars().take(prefix_length).collect()
    }
}

/// Node allows `string | boolean` env values (undefined omitted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
}

impl EnvValue {
    pub fn as_env_string(&self) -> Option<String> {
        match self {
            EnvValue::String(s) => Some(s.clone()),
            EnvValue::Bool(b) => Some(b.to_string()),
            EnvValue::Int(n) => Some(n.to_string()),
            EnvValue::Float(n) => Some(n.to_string()),
        }
    }
}
