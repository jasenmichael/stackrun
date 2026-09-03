//! Stack run: before hooks, optional per-command tunnels, concurrent commands, after hooks.
//!
//! Child spawn lives in [`crate::process`]. Cloudflare ops live in [`crate::tunnel`].

use crate::config::types::{Command, StackrunConfig};
use crate::error::Error;
use crate::logging;
use crate::process::{self, ConcurrentRun};
use crate::tunnel::{self, TunnelRuntime, TunnelSession};

/// Run the stack with the real tunnel backend.
pub fn run(config: &StackrunConfig) -> Result<u8, Error> {
    run_with_tunnel(config, TunnelRuntime::real())
}

/// Same as [`run`] with an injectable tunnel backend (tests).
pub fn run_with_tunnel(config: &StackrunConfig, runtime: TunnelRuntime) -> Result<u8, Error> {
    let tunnel_on = config.tunnel_enabled();
    let cmds = config.runnable_commands();
    let has_local = cmds.iter().any(|c| c.tunnel_local().is_some());

    let host_color = logging::host_color_enabled(config.process.as_ref());

    if tunnel_on {
        logging::emit_opt("Tunneling is enabled", host_color);
        if !has_local {
            return Err(Error::NoTunnelIngress);
        }
        let _binary = runtime.cloudflared.binary_path()?;
        tunnel::unique_named_names(&cmds, &config.tunnel_defaults())?;
        if cmds.iter().any(|c| c.is_named_tunnel())
            && !runtime.cloudflared.has_cert(&tunnel::default_config_dir())
        {
            return Err(Error::CloudflaredLoginRequired {
                dir: tunnel::default_config_dir().display().to_string(),
            });
        }
    } else {
        logging::emit_opt("Tunneling is disabled", host_color);
        if has_local {
            logging::emit_opt(
                "Config has tunnel.local. Pass --tunnel or set TUNNEL=true, or omit tunnel: false, to start cloudflared siblings",
                host_color,
            );
        }
    }

    let exec_env: Vec<(String, String)> = std::env::vars().collect();

    if config.before_commands().is_empty() {
        logging::emit_opt("No before hooks to run", host_color);
    } else {
        logging::emit_opt("Running before hooks", host_color);
        for command in config.before_commands() {
            logging::emit_opt(format!("Running before hook: {command}"), host_color);
            process::run_hook(command, &exec_env, true)?;
        }
    }

    let mut sessions: Vec<TunnelSession> = Vec::new();
    let mut specs = Vec::new();
    let defaults = config.tunnel_defaults();

    if tunnel_on {
        for cmd in &cmds {
            specs.push(cmd.clone());
            match setup_sibling(&runtime, cmd, &defaults) {
                Ok(Some((sibling, session))) => {
                    specs.push(sibling);
                    if let Some(sess) = session {
                        sessions.push(sess);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    for sess in &sessions {
                        tunnel::cleanup(&runtime, sess);
                    }
                    return Err(err);
                }
            }
        }
    } else {
        specs.extend(cmds.iter().cloned());
    }

    if specs.is_empty() {
        return Err(Error::NoCommands);
    }

    for cmd in &cmds {
        logging::emit_opt(command_start_line(cmd), host_color);
        if tunnel_on && cmd.tunnel_local().is_some() {
            logging::emit_opt(tunnel_start_line(cmd, &defaults), host_color);
        }
    }

    let outcome = match process::run_concurrent(ConcurrentRun {
        commands: specs,
        options: config.process.clone().unwrap_or_default(),
        apply_tunnel_env: tunnel_on,
    }) {
        Ok(outcome) => outcome,
        Err(err) => {
            for sess in &sessions {
                tunnel::cleanup(&runtime, sess);
            }
            return Err(err);
        }
    };

    for sess in &sessions {
        tunnel::cleanup(&runtime, sess);
    }

    // after always runs once commands have exited (ok, fail, or Ctrl+C).
    // Empty / omitted after is a no-op and does not change the exit code.
    if config.after_commands().is_empty() {
        logging::emit_opt("No after hooks to run", host_color);
    } else {
        logging::emit_opt("Running after hooks", host_color);
        for command in config.after_commands() {
            logging::emit_opt(format!("Running after hook: {command}"), host_color);
            process::run_hook(command, &exec_env, false)?;
        }
    }

    Ok(outcome.worst_code)
}

fn setup_sibling(
    runtime: &TunnelRuntime,
    cmd: &Command,
    defaults: &crate::config::types::TunnelDefaults,
) -> Result<Option<(Command, Option<TunnelSession>)>, Error> {
    let Some(local) = cmd.tunnel_local() else {
        return Ok(None);
    };
    let binary = runtime.cloudflared.binary_path()?;
    if let Some(public) = cmd.tunnel_public() {
        let name = cmd
            .named_tunnel_name_with(defaults)
            .expect("named tunnel has a name after unique_named_names");
        let remove = cmd
            .tunnel
            .as_ref()
            .and_then(|t| t.remove_existing)
            .or(defaults.remove_existing)
            .unwrap_or(false);
        let sess = tunnel::setup_named(runtime, &name, local, public, remove)?;
        let sibling = Command {
            run: tunnel::named_run_command(&sess),
            name: Some(cmd.sibling_prefix(defaults)),
            color: Some(cmd.sibling_color(defaults)),
            ..Command::default()
        };
        Ok(Some((sibling, Some(sess))))
    } else {
        let sibling = Command {
            run: tunnel::quick_run_command(&binary, local),
            name: Some(cmd.sibling_prefix(defaults)),
            color: Some(cmd.sibling_color(defaults)),
            ..Command::default()
        };
        Ok(Some((sibling, None)))
    }
}

/// Human start line for a user command (stderr, not prefixed child output).
fn command_start_line(cmd: &Command) -> String {
    let name = cmd
        .name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("command");
    match cmd.cwd.as_deref().filter(|s| !s.is_empty()) {
        Some(cwd) => format!("Starting [{name}] {} in {cwd}", cmd.run),
        None => format!("Starting [{name}] {}", cmd.run),
    }
}

/// Human start line for a cloudflared sibling. Named tunnels include `public`.
/// Quick tunnels mention `*.trycloudflare.com` without parsing child stdout.
fn tunnel_start_line(cmd: &Command, defaults: &crate::config::types::TunnelDefaults) -> String {
    let sibling = cmd.sibling_prefix(defaults);
    let name = cmd
        .name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("command");
    let local = cmd.tunnel_local().unwrap_or("");
    match cmd.tunnel_public() {
        Some(public) => format!(
            "Starting tunnel sibling [{sibling}] for {name} at {local} ({public})"
        ),
        None => format!(
            "Starting tunnel sibling [{sibling}] for {name} at {local} (public host is a new *.trycloudflare.com)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::CommandTunnel;
    use crate::logging;

    #[test]
    fn named_sibling_defaults_prefix_not_command_name() {
        let defaults = crate::config::types::TunnelDefaults::default();
        let cmd = Command {
            run: "echo".into(),
            name: Some("api".into()),
            color: Some("green".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:4000".into()),
                public: Some("https://api.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(cmd.named_tunnel_name().as_deref(), Some("api"));
        assert_eq!(cmd.sibling_prefix(&defaults), "tunnel");
        assert_eq!(cmd.sibling_color(&defaults), "cyan");
        assert_eq!(
            tunnel::quick_run_command("cloudflared", "http://127.0.0.1:3000"),
            "cloudflared tunnel --url http://127.0.0.1:3000"
        );
    }

    #[test]
    fn sibling_prefix_and_resource_resolve() {
        let defaults = crate::config::types::TunnelDefaults {
            prefix: Some("tunnel".into()),
            color: Some("cyan".into()),
            resource: Some("bugpin".into()),
            ..crate::config::types::TunnelDefaults::default()
        };
        let cmd = Command {
            run: "echo".into(),
            name: Some("nuxt".into()),
            color: Some("green".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:3000".into()),
                public: Some("https://bugpin.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(cmd.sibling_prefix(&defaults), "tunnel");
        assert_eq!(cmd.sibling_color(&defaults), "cyan");
        assert_eq!(
            cmd.named_tunnel_name_with(&defaults).as_deref(),
            Some("bugpin")
        );
    }

    #[test]
    fn command_start_line_name_run_and_optional_cwd() {
        let cmd = Command {
            run: "nuxt dev --port 3001 --host".into(),
            name: Some("nuxt".into()),
            ..Command::default()
        };
        assert_eq!(
            logging::format_host_line(&command_start_line(&cmd), false),
            "[stackrun] Starting [nuxt] nuxt dev --port 3001 --host"
        );
        let with_cwd = Command {
            cwd: Some("apps/web".into()),
            ..cmd
        };
        assert_eq!(
            logging::format_host_line(&command_start_line(&with_cwd), false),
            "[stackrun] Starting [nuxt] nuxt dev --port 3001 --host in apps/web"
        );
        let unnamed = Command {
            run: "echo hi".into(),
            ..Command::default()
        };
        assert_eq!(
            logging::format_host_line(&command_start_line(&unnamed), false),
            "[stackrun] Starting [command] echo hi"
        );
    }

    #[test]
    fn tunnel_start_line_named_includes_public() {
        let defaults = crate::config::types::TunnelDefaults {
            prefix: Some("tunnel".into()),
            ..crate::config::types::TunnelDefaults::default()
        };
        let cmd = Command {
            run: "nuxt dev --port 3001 --host".into(),
            name: Some("nuxt".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://localhost:3001".into()),
                public: Some("https://app.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(
            logging::format_host_line(&tunnel_start_line(&cmd, &defaults), false),
            "[stackrun] Starting tunnel sibling [tunnel] for nuxt at http://localhost:3001 (https://app.example.dev)"
        );
    }

    #[test]
    fn tunnel_start_line_quick_mentions_trycloudflare() {
        let defaults = crate::config::types::TunnelDefaults::default();
        let cmd = Command {
            run: "echo".into(),
            name: Some("web".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:3000".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(
            logging::format_host_line(&tunnel_start_line(&cmd, &defaults), false),
            "[stackrun] Starting tunnel sibling [tunnel] for web at http://127.0.0.1:3000 (public host is a new *.trycloudflare.com)"
        );
    }
}
