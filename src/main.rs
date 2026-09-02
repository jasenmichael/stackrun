use anyhow::Context;
use clap::Parser;
use stackrun::cli::Cli;
use stackrun::config::{load_config, LoadOptions};
use stackrun::logging;
use stackrun::stack;
use std::process::ExitCode;
use tracing::{error, info};

fn main() -> ExitCode {
    logging::init();
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            error!("{err:#}");
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> anyhow::Result<u8> {
    let cli = Cli::parse();

    if cli.print_version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    let options = LoadOptions::from_cli(&cli);
    let loaded = load_config(options).context("failed to load Stackrun config")?;

    if cli.dry_run {
        println!("{}", stackrun::format_dry_run(&loaded)?);
        return Ok(0);
    }

    if let Some(path) = &loaded.config_file {
        info!(
            "Running stackrun with the loaded config: {}",
            path.display()
        );
    } else {
        info!("Running stackrun without a config file");
    }

    let code = stack::run(&loaded.config)?;
    info!("Stackrun completed");
    Ok(code)
}
