mod cloudflared;
mod dns;
mod session;

use crate::config::types::{CfTunnelConfig, CommandSpec};
use crate::error::Error;
use std::env;
use std::sync::Arc;

pub use cloudflared::{default_config_dir, CloudflaredOps, RealCloudflared};
pub use dns::{DnsApi, RealDns, RecordingDns};
pub use session::{cleanup, run_command_line, setup, TunnelSession};

/// Ingress hostname + local service, generated like historical Node on `main`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingress {
    pub hostname: String,
    pub service: String,
}

#[derive(Clone)]
pub struct TunnelRuntime {
    pub cloudflared: Arc<dyn CloudflaredOps>,
    pub dns: Arc<dyn DnsApi>,
}

impl TunnelRuntime {
    pub fn real() -> Self {
        Self {
            cloudflared: Arc::new(RealCloudflared),
            dns: Arc::new(RealDns::default()),
        }
    }

    pub fn from_parts(
        cloudflared: impl CloudflaredOps + 'static,
        dns: impl DnsApi + 'static,
    ) -> Self {
        Self {
            cloudflared: Arc::new(cloudflared),
            dns: Arc::new(dns),
        }
    }
}

pub fn hostname_from_tunnel_url(tunnel_url: &str) -> String {
    tunnel_url
        .trim()
        .strip_prefix("https://")
        .or_else(|| tunnel_url.trim().strip_prefix("http://"))
        .unwrap_or(tunnel_url.trim())
        .to_string()
}

pub fn ingress_from_commands(commands: &[CommandSpec]) -> Vec<Ingress> {
    commands
        .iter()
        .filter_map(|cmd| {
            let url = cmd.url.as_deref()?;
            let tunnel_url = cmd.tunnel_url.as_deref()?;
            if url.is_empty() || tunnel_url.is_empty() {
                return None;
            }
            Some(Ingress {
                hostname: hostname_from_tunnel_url(tunnel_url),
                service: url.to_string(),
            })
        })
        .collect()
}

pub fn resolve_token(cfg: Option<&CfTunnelConfig>) -> Option<String> {
    cfg.and_then(|c| c.cf_token.clone())
        .or_else(|| env::var("CF_TOKEN").ok())
        .or_else(|| env::var("CLOUDFLARE_TOKEN").ok())
        .filter(|s| !s.is_empty())
}

pub fn resolve_tunnel_name(cfg: Option<&CfTunnelConfig>) -> String {
    cfg.and_then(|c| c.tunnel_name.clone())
        .or_else(|| env::var("CF_TUNNEL_NAME").ok())
        .or_else(|| env::var("CLOUDFLARE_TUNNEL_NAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "stackrun".to_string())
}

/// Validate tunnel inputs the same way Node does (token + at least one ingress).
pub fn prepare(
    cfg: Option<&CfTunnelConfig>,
    commands: &[CommandSpec],
) -> Result<Vec<Ingress>, Error> {
    if resolve_token(cfg).is_none() {
        return Err(Error::CloudflareTokenRequired);
    }
    let ingress = ingress_from_commands(commands);
    if ingress.is_empty() {
        return Err(Error::NoTunnelIngress);
    }
    Ok(ingress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scheme() {
        assert_eq!(
            hostname_from_tunnel_url("https://api.example.dev"),
            "api.example.dev"
        );
        assert_eq!(
            hostname_from_tunnel_url("http://app.example.dev"),
            "app.example.dev"
        );
    }

    #[test]
    fn prepare_requires_token() {
        let cmds = [CommandSpec {
            command: "echo".into(),
            url: Some("http://localhost:1".into()),
            tunnel_url: Some("https://x.example".into()),
            ..CommandSpec::default()
        }];
        let err = prepare(None, &cmds).unwrap_err();
        assert!(matches!(err, Error::CloudflareTokenRequired));
    }
}
