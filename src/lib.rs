//! Stackrun library: config, process orchestration, and tunnel types.
//!
//! Canonical config is [`config::StackrunConfig`]. JS/TS config files are loaded
//! through an optional out-of-process Node + Jiti helper, not by this crate.

pub mod bridge;
pub mod cli;
pub mod config;
pub mod error;
pub mod logging;
pub mod process;
pub mod tunnel;

pub use config::load::{apply_defaults, load_config, LoadOptions, LoadedConfig};
pub use config::types::{
    CfTunnelConfig, CommandEntry, CommandSpec, ConcurrentlyOptions, StackrunConfig,
    TunnelCommandOptions,
};
pub use error::Error;
