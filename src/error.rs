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
        "JS/TS config `{path}` requires Node.js, but `node` was not found on PATH. \
         Install Node.js, or use a YAML, TOML, or JSON config."
    )]
    NodeRequired { path: PathBuf },

    #[error(
        "JS/TS config `{path}` needs jiti in this project. \
         Use a YAML, TOML, or JSON config, run `npm i -D jiti` here, \
         or retry with `--jiti npx` (or `STACKRUN_JITI=npx`)."
    )]
    JitiRequired { path: PathBuf },

    #[error(
        "`npx` was not found on PATH while loading `{path}`. \
         Install Node.js and npm, add jiti in this project (`npm i -D jiti`), \
         or use a YAML, TOML, or JSON config."
    )]
    NpxRequired { path: PathBuf },

    #[error("JS/TS config `{path}` could not be loaded: {message}")]
    JsBridge { path: PathBuf, message: String },

    #[error("Remote config extends are not supported (`{uri}`). Use a local path.")]
    RemoteExtends { uri: String },

    #[error("--tunnel was set but no command has tunnel.local")]
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
        "Tunnel \"{name}\" already exists. Set removeExisting: true to remove it automatically."
    )]
    TunnelAlreadyExists { name: String },

    #[error(
        "Named tunnel name \"{name}\" is used more than once. Set tunnel.resource or use unique command names."
    )]
    DuplicateTunnelName { name: String },

    #[error("cloudflared failed: {message}")]
    Cloudflared { message: String },

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
