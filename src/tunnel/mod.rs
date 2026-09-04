mod cloudflared;
mod session;

use crate::config::types::Command;
use std::sync::Arc;

pub use cloudflared::{
    default_config_dir, CloudflaredOps, MockCloudflared, RealCloudflared, TunnelRow,
};
pub use session::{
    cleanup, hostname_from_public, named_run_argv, named_run_command, quick_run_argv,
    quick_run_command, setup_named, TunnelSession,
};

#[derive(Clone)]
pub struct TunnelRuntime {
    pub cloudflared: Arc<dyn CloudflaredOps>,
}

impl TunnelRuntime {
    pub fn real() -> Self {
        Self {
            cloudflared: Arc::new(RealCloudflared),
        }
    }

    pub fn from_parts(cloudflared: impl CloudflaredOps + 'static) -> Self {
        Self {
            cloudflared: Arc::new(cloudflared),
        }
    }

    pub fn from_arc(cloudflared: Arc<dyn CloudflaredOps>) -> Self {
        Self { cloudflared }
    }
}

/// Unique named-tunnel names among commands that have `public` set.
pub fn unique_named_names(commands: &[Command]) -> Result<(), crate::error::Error> {
    let mut seen = std::collections::BTreeSet::new();
    for cmd in commands {
        if let Some(name) = cmd.named_tunnel_name() {
            if !seen.insert(name.clone()) {
                return Err(crate::error::Error::DuplicateTunnelName { name });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::CommandTunnel;
    use crate::error::Error;

    #[test]
    fn strips_scheme() {
        assert_eq!(
            hostname_from_public("https://api.example.dev"),
            "api.example.dev"
        );
        assert_eq!(
            hostname_from_public("http://app.example.dev"),
            "app.example.dev"
        );
    }

    #[test]
    fn duplicate_named_names_error() {
        let cmds = [
            Command {
                run: "echo".into(),
                name: Some("api".into()),
                tunnel: Some(CommandTunnel {
                    local: Some("http://127.0.0.1:1".into()),
                    public: Some("https://a.example".into()),
                    ..CommandTunnel::default()
                }),
                ..Command::default()
            },
            Command {
                run: "echo".into(),
                name: Some("api".into()),
                tunnel: Some(CommandTunnel {
                    local: Some("http://127.0.0.1:2".into()),
                    public: Some("https://b.example".into()),
                    ..CommandTunnel::default()
                }),
                ..Command::default()
            },
        ];
        let err = unique_named_names(&cmds).unwrap_err();
        assert!(matches!(err, Error::DuplicateTunnelName { .. }));
    }
}
