use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Canonical Stackrun configuration.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StackrunConfig {
    pub before: Option<Vec<String>>,
    pub after: Option<Vec<String>>,
    pub process: Option<ProcessOptions>,
    /// `false` disables. Object is named-tunnel defaults. Omitted infers from `tunnel.local`.
    pub tunnel: Option<TunnelSetting>,
    pub commands: Option<Vec<CommandEntry>>,
    /// Set by `--tunnel` / `TUNNEL=true`. Not a config-file key.
    pub force_tunnel: bool,
}

impl StackrunConfig {
    pub fn tunnel_enabled(&self) -> bool {
        if self.force_tunnel {
            return true;
        }
        match &self.tunnel {
            Some(TunnelSetting::Flag(false)) => false,
            Some(TunnelSetting::Flag(true)) => true,
            Some(TunnelSetting::Defaults(_)) | None => self.has_any_tunnel_local(),
        }
    }

    pub fn tunnel_defaults(&self) -> TunnelDefaults {
        match &self.tunnel {
            Some(TunnelSetting::Defaults(d)) => d.clone(),
            _ => TunnelDefaults::default(),
        }
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

    /// Commands that have a non-empty `run` (entries without a string command are filtered).
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

impl Serialize for StackrunConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("StackrunConfig", 5)?;
        state.serialize_field("before", &self.before)?;
        state.serialize_field("after", &self.after)?;
        state.serialize_field("process", &self.process)?;
        state.serialize_field("tunnel", &self.tunnel)?;
        state.serialize_field("commands", &self.commands)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for StackrunConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = StackrunConfigRaw::deserialize(deserializer)?;
        Ok(raw.into())
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StackrunConfigRaw {
    before: Option<Vec<String>>,
    before_commands: Option<Vec<String>>,
    after: Option<Vec<String>>,
    after_commands: Option<Vec<String>>,
    process: Option<ProcessOptions>,
    concurrently_options: Option<ProcessOptions>,
    tunnel: Option<TunnelSetting>,
    tunnel_enabled: Option<bool>,
    cf_tunnel_config: Option<LegacyCfTunnelConfig>,
    commands: Option<Vec<CommandEntry>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct LegacyCfTunnelConfig {
    remove_existing: Option<bool>,
    remove_existing_tunnel: Option<bool>,
    remove_existing_dns: Option<bool>,
    tunnel_name: Option<String>,
    command_options: Option<LegacyCommandOptions>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct LegacyCommandOptions {
    name: Option<String>,
    prefix_color: Option<String>,
}

impl From<StackrunConfigRaw> for StackrunConfig {
    fn from(raw: StackrunConfigRaw) -> Self {
        let before = raw.before.or(raw.before_commands);
        let after = raw.after.or(raw.after_commands);
        let process = raw.process.or(raw.concurrently_options);
        let legacy = TunnelDefaults::from_legacy(raw.cf_tunnel_config.as_ref());

        let tunnel = match raw.tunnel {
            Some(TunnelSetting::Flag(false)) => Some(TunnelSetting::Flag(false)),
            Some(TunnelSetting::Flag(true)) => Some(legacy.into_setting_or_flag(true)),
            Some(TunnelSetting::Defaults(d)) => {
                Some(TunnelSetting::Defaults(d.merge_missing(&legacy)))
            }
            None => match raw.tunnel_enabled {
                Some(false) => Some(TunnelSetting::Flag(false)),
                Some(true) => Some(legacy.into_setting_or_flag(true)),
                None if legacy.is_empty() => None,
                None => Some(TunnelSetting::Defaults(legacy)),
            },
        };

        StackrunConfig {
            before,
            after,
            process,
            tunnel,
            commands: raw.commands,
            force_tunnel: false,
        }
    }
}

/// Stack-level tunnel: `false` / `true` or named-tunnel defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TunnelSetting {
    Flag(bool),
    Defaults(TunnelDefaults),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_existing: Option<bool>,
    /// Sibling log prefix. Default `Tunnel`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Sibling prefix color. Default `cyan`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Default Cloudflare object name for named tunnels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
}

const DEFAULT_SIBLING_PREFIX: &str = "Tunnel";
const DEFAULT_SIBLING_COLOR: &str = "cyan";

fn nonempty_owned(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

impl TunnelDefaults {
    fn from_legacy(cf: Option<&LegacyCfTunnelConfig>) -> Self {
        let Some(cf) = cf else {
            return Self::default();
        };
        let opts = cf.command_options.as_ref();
        Self {
            remove_existing: cf
                .remove_existing
                .or(cf.remove_existing_tunnel)
                .or(cf.remove_existing_dns),
            prefix: opts.and_then(|o| nonempty_owned(o.name.clone())),
            color: opts.and_then(|o| nonempty_owned(o.prefix_color.clone())),
            resource: nonempty_owned(cf.tunnel_name.clone()),
        }
    }

    fn is_empty(&self) -> bool {
        self.remove_existing.is_none()
            && self.prefix.is_none()
            && self.color.is_none()
            && self.resource.is_none()
    }

    fn merge_missing(mut self, other: &Self) -> Self {
        if self.remove_existing.is_none() {
            self.remove_existing = other.remove_existing;
        }
        if self.prefix.is_none() {
            self.prefix = other.prefix.clone();
        }
        if self.color.is_none() {
            self.color = other.color.clone();
        }
        if self.resource.is_none() {
            self.resource = other.resource.clone();
        }
        self
    }

    fn into_setting_or_flag(self, flag: bool) -> TunnelSetting {
        if self.is_empty() {
            TunnelSetting::Flag(flag)
        } else {
            TunnelSetting::Defaults(self)
        }
    }
}

/// Process options Stackrun honors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProcessOptions {
    pub kill_others: Option<KillOthers>,
    pub handle_input: Option<bool>,
    #[serde(alias = "prefixColors")]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub run: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, EnvValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tunnel: Option<CommandTunnel>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandTunnel {
    #[serde(alias = "url", skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    #[serde(alias = "tunnelUrl", skip_serializing_if = "Option::is_none")]
    pub public: Option<String>,
    #[serde(alias = "tunnelEnv", skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, EnvValue>>,
    /// Cloudflare object name. Alias: `name`.
    #[serde(alias = "name", skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Sibling log prefix. Falls back to stack `tunnel.prefix`, then `Tunnel`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Sibling prefix color. Falls back to stack `tunnel.color`, then `cyan`.
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

    /// Named-tunnel Cloudflare object name. Defaults to `command.name`.
    pub fn named_tunnel_name(&self) -> Option<String> {
        self.named_tunnel_name_with(&TunnelDefaults::default())
    }

    /// Cloudflare object name: command `resource`/`name`, else stack `resource`,
    /// else `command.name`, else `stackrun`.
    pub fn named_tunnel_name_with(&self, defaults: &TunnelDefaults) -> Option<String> {
        if !self.is_named_tunnel() {
            return None;
        }
        Some(
            self.tunnel
                .as_ref()
                .and_then(|t| nonempty_owned(t.resource.clone()))
                .or_else(|| nonempty_owned(defaults.resource.clone()))
                .or_else(|| nonempty_owned(self.name.clone()))
                .unwrap_or_else(|| "stackrun".to_string()),
        )
    }

    /// Log prefix for the cloudflared sibling.
    pub fn sibling_prefix(&self, defaults: &TunnelDefaults) -> String {
        self.tunnel
            .as_ref()
            .and_then(|t| nonempty_owned(t.prefix.clone()))
            .or_else(|| nonempty_owned(defaults.prefix.clone()))
            .unwrap_or_else(|| DEFAULT_SIBLING_PREFIX.to_string())
    }

    /// Prefix color for the cloudflared sibling.
    pub fn sibling_color(&self, defaults: &TunnelDefaults) -> String {
        self.tunnel
            .as_ref()
            .and_then(|t| nonempty_owned(t.color.clone()))
            .or_else(|| nonempty_owned(defaults.color.clone()))
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

    pub fn display_name(&self, prefix_length: usize) -> String {
        let raw = self.name.clone().unwrap_or_default();
        if prefix_length == 0 || raw.is_empty() {
            return raw;
        }
        raw.chars().take(prefix_length).collect()
    }
}

impl<'de> Deserialize<'de> for Command {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CommandVisitor;

        impl<'de> Visitor<'de> for CommandVisitor {
            type Value = Command;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a stackrun command object")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Command, M::Error> {
                let mut run: Option<String> = None;
                let mut name = None;
                let mut cwd = None;
                let mut env = None;
                let mut color = None;
                let mut tunnel: Option<CommandTunnel> = None;
                let mut url = None;
                let mut tunnel_url = None;
                let mut tunnel_env = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "run" | "command" => {
                            run = Some(map.next_value()?);
                        }
                        "name" => name = Some(map.next_value()?),
                        "cwd" => cwd = Some(map.next_value()?),
                        "env" => env = Some(map.next_value()?),
                        "color" | "prefixColor" => color = Some(map.next_value()?),
                        "tunnel" => tunnel = Some(map.next_value()?),
                        "url" => url = Some(map.next_value()?),
                        "tunnelUrl" => tunnel_url = Some(map.next_value()?),
                        "tunnelEnv" => tunnel_env = Some(map.next_value()?),
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let tunnel = match tunnel {
                    Some(mut t) => {
                        if t.local.is_none() {
                            t.local = url;
                        }
                        if t.public.is_none() {
                            t.public = tunnel_url;
                        }
                        if t.env.is_none() {
                            t.env = tunnel_env;
                        }
                        Some(t)
                    }
                    None if url.is_some() || tunnel_url.is_some() || tunnel_env.is_some() => {
                        Some(CommandTunnel {
                            local: url,
                            public: tunnel_url,
                            env: tunnel_env,
                            ..CommandTunnel::default()
                        })
                    }
                    None => None,
                };

                Ok(Command {
                    run: run.unwrap_or_default(),
                    name,
                    cwd,
                    env,
                    color,
                    tunnel,
                })
            }
        }

        deserializer.deserialize_map(CommandVisitor)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_tunnel_name_is_resource_alias() {
        let t: CommandTunnel =
            serde_json::from_str(r#"{"local":"http://localhost:1","name":"bugpin"}"#).unwrap();
        assert_eq!(t.resource.as_deref(), Some("bugpin"));
    }

    #[test]
    fn sibling_defaults_are_tunnel_cyan() {
        let cmd = Command {
            name: Some("nuxt".into()),
            run: "echo".into(),
            color: Some("green".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://localhost:3000".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        let defaults = TunnelDefaults::default();
        assert_eq!(cmd.sibling_prefix(&defaults), "Tunnel");
        assert_eq!(cmd.sibling_color(&defaults), "cyan");
    }

    #[test]
    fn command_prefix_wins_over_stack() {
        let cmd = Command {
            name: Some("nuxt".into()),
            run: "echo".into(),
            tunnel: Some(CommandTunnel {
                local: Some("http://localhost:3000".into()),
                prefix: Some("edge".into()),
                color: Some("magenta".into()),
                resource: Some("cmd-cf".into()),
                public: Some("https://x.example".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        let defaults = TunnelDefaults {
            prefix: Some("tunnel".into()),
            color: Some("cyan".into()),
            resource: Some("stack-cf".into()),
            ..TunnelDefaults::default()
        };
        assert_eq!(cmd.sibling_prefix(&defaults), "edge");
        assert_eq!(cmd.sibling_color(&defaults), "magenta");
        assert_eq!(
            cmd.named_tunnel_name_with(&defaults).as_deref(),
            Some("cmd-cf")
        );
    }

    #[test]
    fn legacy_cftunnel_maps_prefix_color_resource() {
        let cfg: StackrunConfig = serde_json::from_str(
            r#"{
                "cfTunnelConfig": {
                    "tunnelName": "bugpin",
                    "removeExistingTunnel": true,
                    "commandOptions": { "name": "tunnel", "prefixColor": "cyan" }
                },
                "commands": [{
                    "name": "nuxt",
                    "command": "echo nuxt",
                    "url": "http://localhost:3000",
                    "tunnelUrl": "https://bugpin.example.dev"
                }]
            }"#,
        )
        .unwrap();
        let defaults = cfg.tunnel_defaults();
        assert_eq!(defaults.prefix.as_deref(), Some("tunnel"));
        assert_eq!(defaults.color.as_deref(), Some("cyan"));
        assert_eq!(defaults.resource.as_deref(), Some("bugpin"));
        assert_eq!(defaults.remove_existing, Some(true));
        let cmd = &cfg.runnable_commands()[0];
        assert_eq!(cmd.sibling_prefix(&defaults), "tunnel");
        assert_eq!(cmd.sibling_color(&defaults), "cyan");
        assert_eq!(
            cmd.named_tunnel_name_with(&defaults).as_deref(),
            Some("bugpin")
        );
        assert_ne!(cmd.sibling_prefix(&defaults), "nuxt");
    }
}
