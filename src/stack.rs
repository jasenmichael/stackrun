//! Stack run: before hooks, optional per-command tunnels, concurrent commands, after hooks.
//!
//! Child spawn lives in [`crate::process`]. Cloudflare ops live in [`crate::tunnel`].

use crate::config::types::{Command, StackrunConfig};
use crate::error::Error;
use crate::process::{self, ConcurrentRun};
use crate::tunnel::{self, TunnelRuntime, TunnelSession};
use tracing::info;

/// Run the stack with the real tunnel backend.
pub fn run(config: &StackrunConfig) -> Result<u8, Error> {
    run_with_tunnel(config, TunnelRuntime::real())
}

/// Same as [`run`] with an injectable tunnel backend (tests).
pub fn run_with_tunnel(config: &StackrunConfig, runtime: TunnelRuntime) -> Result<u8, Error> {
    let tunnel_on = config.tunnel_enabled();
    let cmds = config.runnable_commands();
    let has_local = cmds.iter().any(|c| c.tunnel_local().is_some());

    if tunnel_on {
        info!("Tunneling is enabled");
        eprintln!("Tunneling is enabled");
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
        info!("Tunneling is disabled");
        eprintln!("Tunneling is disabled");
        if has_local {
            let msg = "Config has tunnel.local. Pass --tunnel or set TUNNEL=true, or omit tunnel: false, to start cloudflared siblings";
            info!("{msg}");
            eprintln!("{msg}");
        }
    }

    let exec_env: Vec<(String, String)> = std::env::vars().collect();

    if config.before_commands().is_empty() {
        info!("No before hooks to run");
    } else {
        info!("Running before hooks");
        for command in config.before_commands() {
            info!("Running before hook: {command}");
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
                    eprintln!(
                        "Starting tunnel sibling [{}] for {} at {}",
                        sibling.name.as_deref().unwrap_or("Tunnel"),
                        cmd.name.as_deref().unwrap_or("command"),
                        cmd.tunnel_local().unwrap_or(""),
                    );
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
        specs.extend(cmds);
    }

    if specs.is_empty() {
        return Err(Error::NoCommands);
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

    // Ctrl+C stops every concurrent command, then `after` still runs.
    // A failed command (no interrupt) still skips `after`.
    if config.after_commands().is_empty() {
        info!("No after hooks to run");
    } else if outcome.interrupted || outcome.worst_code == 0 {
        info!("Running after hooks");
        for command in config.after_commands() {
            info!("Running after hook: {command}");
            process::run_hook(command, &exec_env, false)?;
        }
    }

    if outcome.interrupted {
        return Ok(if outcome.worst_code == 0 {
            1
        } else {
            outcome.worst_code
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::CommandTunnel;

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
        assert_eq!(cmd.sibling_prefix(&defaults), "Tunnel");
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
}
