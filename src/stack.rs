//! Stack run: beforeCommands, optional tunnel, concurrent commands, afterCommands.
//!
//! Child spawn lives in [`crate::process`]. Cloudflare ops live in [`crate::tunnel`].

use crate::config::types::{Command, StackrunConfig, TunnelCommandOptions};
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
    let tunnel_enabled = config.tunnel_enabled();
    let cmds = config.runnable_commands();
    let mut pending_ingress = None;
    if tunnel_enabled {
        info!("Tunneling is enabled");
        pending_ingress = Some(tunnel::prepare(config.cf_tunnel_config.as_ref(), &cmds)?);
    } else {
        info!("Tunneling is disabled");
        if !tunnel::ingress_from_commands(&cmds).is_empty() {
            info!("Config has url/tunnelUrl pairs. Pass --tunnel or set TUNNEL=true to start cloudflared as a sibling process");
        }
    }

    let exec_env: Vec<(String, String)> = std::env::vars().collect();

    if config.before_commands().is_empty() {
        info!("No beforeCommands to run");
    } else {
        info!("Running beforeCommands");
        for command in config.before_commands() {
            info!("Running beforeCommand: {command}");
            process::run_hook(command, &exec_env, true)?;
        }
    }

    let mut session: Option<TunnelSession> = None;
    if let Some(ingress) = pending_ingress {
        let token = tunnel::resolve_token(config.cf_tunnel_config.as_ref())
            .ok_or(Error::CloudflareTokenRequired)?;
        session = Some(tunnel::setup(
            &runtime,
            config.cf_tunnel_config.as_ref(),
            ingress,
            token,
        )?);
    }

    let mut specs = cmds;
    if let Some(ref sess) = session {
        let opts = config
            .cf_tunnel_config
            .as_ref()
            .and_then(|c| c.command_options.as_ref());
        specs.insert(0, tunnel_command(sess, opts));
    }
    if specs.is_empty() {
        return Err(Error::NoCommands);
    }

    let outcome = match process::run_concurrent(ConcurrentRun {
        commands: specs,
        options: config.process_options.clone().unwrap_or_default(),
        apply_tunnel_env: tunnel_enabled,
    }) {
        Ok(outcome) => outcome,
        Err(err) => {
            if let Some(ref sess) = session {
                tunnel::cleanup(&runtime, sess);
            }
            return Err(err);
        }
    };

    if let Some(ref sess) = session {
        tunnel::cleanup(&runtime, sess);
    }

    // Ctrl+C stops every concurrent command, then afterCommands still run.
    // A failed command (no interrupt) still skips afterCommands.
    if config.after_commands().is_empty() {
        info!("No afterCommands to run");
    } else if outcome.interrupted || outcome.worst_code == 0 {
        info!("Running afterCommands");
        for command in config.after_commands() {
            info!("Running afterCommand: {command}");
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

/// Tunnel sibling is a normal [`Command`]. Display fields come from
/// `cfTunnelConfig.commandOptions`, not from [`TunnelSession`].
fn tunnel_command(sess: &TunnelSession, opts: Option<&TunnelCommandOptions>) -> Command {
    Command {
        command: tunnel::run_command_line(sess),
        name: Some(
            opts.and_then(|o| o.name.clone())
                .unwrap_or_else(|| "Tunnel".into()),
        ),
        cwd: opts.and_then(|o| o.cwd.clone()),
        prefix_color: opts
            .and_then(|o| o.prefix_color.clone())
            .or_else(|| Some("cyan".into())),
        env: opts.and_then(|o| o.env.clone()),
        ..Command::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::EnvValue;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn session() -> TunnelSession {
        TunnelSession {
            config_dir: PathBuf::from("/tmp"),
            tunnel_name: "stackrun".into(),
            tunnel_id: "id".into(),
            ingress: vec![],
            token: "tok".into(),
            binary: "cloudflared".into(),
        }
    }

    #[test]
    fn tunnel_command_defaults_name_and_cyan() {
        let cmd = tunnel_command(&session(), None);
        assert_eq!(cmd.command, "cloudflared tunnel run stackrun");
        assert_eq!(cmd.name.as_deref(), Some("Tunnel"));
        assert_eq!(cmd.prefix_color.as_deref(), Some("cyan"));
        assert!(cmd.cwd.is_none());
        assert!(cmd.env.is_none());
    }

    #[test]
    fn tunnel_command_uses_command_options() {
        let mut env = BTreeMap::new();
        env.insert("K".into(), EnvValue::String("v".into()));
        let opts = TunnelCommandOptions {
            name: Some("tunnel".into()),
            prefix_color: Some("magenta".into()),
            cwd: Some("/work".into()),
            env: Some(env),
            ..TunnelCommandOptions::default()
        };
        let cmd = tunnel_command(&session(), Some(&opts));
        assert_eq!(cmd.name.as_deref(), Some("tunnel"));
        assert_eq!(cmd.prefix_color.as_deref(), Some("magenta"));
        assert_eq!(cmd.cwd.as_deref(), Some("/work"));
        assert_eq!(
            cmd.env.as_ref().unwrap().get("K").unwrap().as_env_string(),
            Some("v".into())
        );
    }
}
