use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical Stackrun configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackrunConfig {
    pub before: Option<Vec<String>>,
    pub after: Option<Vec<String>>,
    pub process: Option<ProcessOptions>,
    pub commands: Option<Vec<CommandEntry>>,
    /// Set by `--tunnel` / `TUNNEL=true`. Not a config-file key.
    #[serde(skip_deserializing, default)]
    pub force_tunnel: bool,
}

impl StackrunConfig {
    pub fn tunnel_enabled(&self) -> bool {
        self.force_tunnel || self.has_any_tunnel_local()
    }

    /// True when a command has a non-empty `tunnel.local`.
    pub fn has_any_tunnel_local(&self) -> bool {
        self.runnable_commands()
            .iter()
            .any(|c| c.tunnel_local().is_some())
    }

    pub fn before_commands(&self) -> &[String] {
        self.before.as_deref().unwrap_or(&[])
    }

    pub fn after_commands(&self) -> &[String] {
        self.after.as_deref().unwrap_or(&[])
    }

    /// Commands that have a non-empty `run` (entries without a string `run` are filtered).
    pub fn runnable_commands(&self) -> Vec<Command> {
        self.commands
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(CommandEntry::to_command)
            .filter(|c| !c.run.is_empty())
            .collect()
    }
}

const DEFAULT_SIBLING_COLOR: &str = "cyan";

fn nonempty_owned(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

/// Process options Stackrun honors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProcessOptions {
    pub kill_others: Option<KillOthers>,
    pub handle_input: Option<bool>,
    pub colors: Option<PrefixColors>,
    pub prefix_length: Option<u32>,
    pub cwd: Option<String>,
}

impl ProcessOptions {
    pub fn prefix_length_or_default(&self) -> usize {
        self.prefix_length.unwrap_or(10) as usize
    }

    /// Missing/falsy `killOthers` becomes `"failure"`.
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

    /// Default working directory when a command omits `cwd`.
    pub fn cwd_or_none(&self) -> Option<&str> {
        self.cwd.as_deref().filter(|s| !s.is_empty())
    }

    /// Color for a command at `index`. Explicit `color` wins. Otherwise
    /// cycle `colors` / `"auto"`. `colors: false` means no color.
    pub fn resolve_prefix_color(&self, explicit: Option<&str>, index: usize) -> Option<String> {
        if let Some(c) = explicit {
            if !c.is_empty() {
                return Some(c.to_string());
            }
        }
        match &self.colors {
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
#[allow(clippy::large_enum_variant)]
pub enum CommandEntry {
    Shell(String),
    Full(Command),
}

impl CommandEntry {
    pub fn to_command(&self) -> Option<Command> {
        match self {
            CommandEntry::Shell(run) if !run.is_empty() => Some(Command {
                run: run.clone(),
                ..Command::default()
            }),
            CommandEntry::Full(command) if !command.run.is_empty() => Some(command.clone()),
            _ => None,
        }
    }
}

/// One stack command (a concurrent OS process).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Command {
    pub run: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Log prefix. When set, used as-is (not sliced). Else `name` (sliced).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, EnvValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tunnel: Option<CommandTunnel>,
    /// Argv spawn for internal siblings (cloudflared). Not a config key.
    #[serde(skip)]
    pub argv: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandTunnel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, EnvValue>>,
    /// Cloudflare object name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Sibling prefix color. Default `cyan`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_existing: Option<bool>,
}

impl Command {
    pub fn tunnel_local(&self) -> Option<&str> {
        self.tunnel
            .as_ref()
            .and_then(|t| t.local.as_deref())
            .filter(|s| !s.is_empty())
    }

    pub fn tunnel_public(&self) -> Option<&str> {
        self.tunnel
            .as_ref()
            .and_then(|t| t.public.as_deref())
            .filter(|s| !s.is_empty())
    }

    pub fn is_named_tunnel(&self) -> bool {
        self.tunnel_local().is_some() && self.tunnel_public().is_some()
    }

    pub fn is_quick_tunnel(&self) -> bool {
        self.tunnel_local().is_some() && self.tunnel_public().is_none()
    }

    /// `prefix` if set, else `name`.
    pub fn resolved_prefix_raw(&self) -> Option<String> {
        nonempty_owned(self.prefix.clone()).or_else(|| nonempty_owned(self.name.clone()))
    }

    /// Named-tunnel Cloudflare object name: `resource`, else prefix/name, else `stackrun`.
    pub fn named_tunnel_name(&self) -> Option<String> {
        if !self.is_named_tunnel() {
            return None;
        }
        Some(
            self.tunnel
                .as_ref()
                .and_then(|t| nonempty_owned(t.resource.clone()))
                .or_else(|| self.resolved_prefix_raw())
                .unwrap_or_else(|| "stackrun".to_string()),
        )
    }

    /// Cloudflared sibling log token: `tunnel-{display_name}`. Not sliced again.
    pub fn sibling_log_prefix(&self, prefix_length: usize) -> String {
        let base = {
            let d = self.display_name(prefix_length);
            if d.is_empty() {
                "command".to_string()
            } else {
                d
            }
        };
        format!("tunnel-{base}")
    }

    /// Prefix color for the cloudflared sibling.
    pub fn sibling_color(&self) -> String {
        self.tunnel
            .as_ref()
            .and_then(|t| nonempty_owned(t.color.clone()))
            .unwrap_or_else(|| DEFAULT_SIBLING_COLOR.to_string())
    }

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
            if let Some(env) = self.tunnel.as_ref().and_then(|t| t.env.as_ref()) {
                for (k, v) in env {
                    if let Some(s) = v.as_env_string() {
                        out.insert(k.clone(), s);
                    }
                }
            }
        }
        out
    }

    /// Per-command `cwd`, else `process.cwd`.
    pub fn effective_cwd(&self, process: Option<&ProcessOptions>) -> Option<String> {
        nonempty_owned(self.cwd.clone())
            .or_else(|| process.and_then(|p| nonempty_owned(p.cwd.clone())))
    }

    pub fn display_name(&self, prefix_length: usize) -> String {
        if let Some(p) = nonempty_owned(self.prefix.clone()) {
            return p;
        }
        let raw = self.name.clone().unwrap_or_default();
        if prefix_length == 0 || raw.is_empty() {
            return raw;
        }
        raw.chars().take(prefix_length).collect()
    }
}

/// Env values may be `string | boolean | number`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_tunnel_resource_field() {
        let t: CommandTunnel =
            serde_json::from_str(r#"{"local":"http://localhost:1","resource":"bugpin"}"#).unwrap();
        assert_eq!(t.resource.as_deref(), Some("bugpin"));
    }

    #[test]
    fn sibling_log_prefix_uses_command_name() {
        let cmd = Command {
            name: Some("web".into()),
            run: "echo".into(),
            color: Some("green".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://localhost:3000".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(cmd.sibling_log_prefix(10), "tunnel-web");
        assert_eq!(cmd.sibling_color(), "cyan");
    }

    #[test]
    fn command_prefix_wins_over_name_for_sibling() {
        let cmd = Command {
            name: Some("nuxt".into()),
            prefix: Some("edge".into()),
            run: "echo".into(),
            tunnel: Some(CommandTunnel {
                local: Some("http://localhost:3000".into()),
                color: Some("magenta".into()),
                resource: Some("cmd-cf".into()),
                public: Some("https://x.example".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(cmd.display_name(10), "edge");
        assert_eq!(cmd.sibling_log_prefix(10), "tunnel-edge");
        assert_eq!(cmd.sibling_color(), "magenta");
        assert_eq!(cmd.named_tunnel_name().as_deref(), Some("cmd-cf"));
    }

    #[test]
    fn stack_tunnel_key_is_ignored() {
        let cfg: StackrunConfig = serde_json::from_str(
            r#"{
                "tunnel": {
                    "resource": "bugpin",
                    "removeExisting": true
                },
                "commands": [{
                    "name": "nuxt",
                    "run": "echo nuxt",
                    "color": "green",
                    "tunnel": {
                        "local": "http://localhost:3000",
                        "public": "https://bugpin.example.dev",
                        "resource": "bugpin"
                    }
                }]
            }"#,
        )
        .unwrap();
        assert!(cfg.tunnel_enabled());
        let cmd = &cfg.runnable_commands()[0];
        assert_eq!(cmd.sibling_log_prefix(10), "tunnel-nuxt");
        assert_eq!(cmd.sibling_color(), "cyan");
        assert_eq!(cmd.named_tunnel_name().as_deref(), Some("bugpin"));
    }

    #[test]
    fn effective_cwd_prefers_command_then_process() {
        let cmd = Command {
            run: "echo".into(),
            cwd: Some("apps/web".into()),
            ..Command::default()
        };
        let process = ProcessOptions {
            cwd: Some("apps".into()),
            ..ProcessOptions::default()
        };
        assert_eq!(
            cmd.effective_cwd(Some(&process)).as_deref(),
            Some("apps/web")
        );
        let bare = Command {
            run: "echo".into(),
            ..Command::default()
        };
        assert_eq!(bare.effective_cwd(Some(&process)).as_deref(), Some("apps"));
        assert_eq!(bare.effective_cwd(None), None);
    }
}
