//! Stackrun library: config, stack run, process orchestration, and tunnel types.
//!
//! Canonical config is [`config::StackrunConfig`]. JS/TS config files are loaded
//! through an optional out-of-process Node + Jiti helper, not by this crate.

pub(crate) mod bridge;
pub mod cli;
pub mod config;
pub mod error;
pub mod logging;
pub mod process;
pub mod stack;
pub mod tunnel;

pub use config::types::{Command, CommandEntry, CommandTunnel, ProcessOptions, StackrunConfig};
pub use config::{
    apply_defaults, dry_run_report, format_dry_run, load_config, DryRunReport, LoadOptions,
    LoadedConfig,
};
pub use error::Error;
pub use stack::{run, run_with_tunnel};
