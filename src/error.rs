use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("No config file found")]
    NoConfigPath,

    #[error("No valid configuration found at {path}")]
    ConfigNotFound { path: String },

    #[error("Failed to parse {path} as {format}: {message}")]
    Parse {
        path: String,
        format: &'static str,
        message: String,
    },

    #[error(
        "JS/TS config `{path}` requires Node.js to load via Jiti, but `node` was not found on PATH. \
         Install Node.js, or use a native config file (YAML, TOML, JSON, JSONC, JSON5)."
    )]
    NodeRequired { path: PathBuf },

    #[error("JS/TS config `{path}` could not be loaded: {message}")]
    JsBridge { path: PathBuf, message: String },

    #[error("Remote config extends are not supported (`{uri}`). Use a local path.")]
    RemoteExtends { uri: String },

    #[error("Cloudflare token is required for tunneling")]
    CloudflareTokenRequired,

    #[error("No valid tunnel configurations found")]
    NoTunnelIngress,

    #[error(
        "`cloudflared` was not found on PATH. Install it from https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation/ then retry."
    )]
    CloudflaredMissing,

    #[error(
        "cloudflared is not logged in (missing cert.pem in {dir}). Run `cloudflared tunnel login` once, then retry."
    )]
    CloudflaredLoginRequired { dir: String },

    #[error(
        "Tunnel \"{name}\" already exists. Set removeExistingTunnel: true to remove it automatically."
    )]
    TunnelAlreadyExists { name: String },

    #[error(
        "DNS record for \"{hostname}\" already exists. Set removeExistingDns: true to remove it automatically."
    )]
    DnsRecordExists { hostname: String },

    #[error("cloudflared failed: {message}")]
    Cloudflared { message: String },

    #[error("Cloudflare API error: {message}")]
    CloudflareApi { message: String },

    #[error("beforeCommand failed (`{command}`): {status}")]
    BeforeCommandFailed { command: String, status: ExitStatus },

    #[error("afterCommand failed (`{command}`): {status}")]
    AfterCommandFailed { command: String, status: ExitStatus },

    #[error("No commands to run")]
    NoCommands,

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("{0}")]
    Message(String),
}
