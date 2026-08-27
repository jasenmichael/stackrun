use crate::config::load::apply_defaults;
use crate::config::types::{CommandSpec, ConcurrentlyOptions, StackrunConfig};
use crate::error::Error;
use crate::tunnel::{self, TunnelRuntime, TunnelSession};
use owo_colors::OwoColorize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::info;

/// Run beforeCommands, concurrent commands (plus optional tunnel), afterCommands.
pub fn run(config: &StackrunConfig) -> Result<u8, Error> {
    run_with_tunnel(config, TunnelRuntime::real())
}

/// Same as [`run`] with an injectable tunnel backend (tests).
pub fn run_with_tunnel(config: &StackrunConfig, runtime: TunnelRuntime) -> Result<u8, Error> {
    let mut config = config.clone();
    apply_defaults(&mut config);

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
            run_hook(command, &exec_env, true)?;
        }
    }

    let prefix_length = config
        .concurrently_options
        .as_ref()
        .map(|o| o.prefix_length_or_default())
        .unwrap_or(10);
    let kill_on_failure = config
        .concurrently_options
        .as_ref()
        .map(|o| o.kill_others_on_failure())
        .unwrap_or(true);
    let handle_input = config
        .concurrently_options
        .as_ref()
        .map(|o| o.handle_input_or_default())
        .unwrap_or(true);
    let conc_opts = config.concurrently_options.clone().unwrap_or_default();

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
    if specs.is_empty() && session.is_none() {
        return Err(Error::NoCommands);
    }

    let failed = Arc::new(AtomicBool::new(false));
    let shutting_down = Arc::new(AtomicBool::new(false));
    let children: Arc<Mutex<Vec<ChildHandle>>> = Arc::new(Mutex::new(Vec::new()));

    {
        let shutting_down = Arc::clone(&shutting_down);
        let children = Arc::clone(&children);
        let _ = ctrlc::set_handler(move || {
            shutting_down.store(true, Ordering::SeqCst);
            kill_all(&children);
        });
    }

    let mut joins = Vec::new();
    let mut index = 0usize;

    if let Some(ref sess) = session {
        let spec = CommandSpec {
            command: tunnel::run_command_line(sess),
            name: Some(sess.run_name.clone()),
            cwd: sess.run_cwd.clone(),
            prefix_color: sess.run_color.clone(),
            env: config
                .cf_tunnel_config
                .as_ref()
                .and_then(|c| c.command_options.as_ref())
                .and_then(|o| o.env.clone()),
            ..CommandSpec::default()
        };
        spawn_one(
            index,
            spec,
            prefix_length,
            &conc_opts,
            false,
            handle_input,
            kill_on_failure,
            &failed,
            &shutting_down,
            &children,
            &mut joins,
        );
        index += 1;
    }

    for spec in specs.drain(..) {
        spawn_one(
            index,
            spec,
            prefix_length,
            &conc_opts,
            tunnel_enabled,
            handle_input,
            kill_on_failure,
            &failed,
            &shutting_down,
            &children,
            &mut joins,
        );
        index += 1;
    }

    let mut worst: u8 = 0;
    for join in joins {
        match join.join() {
            Ok(Ok(code)) => {
                if code != 0 && worst == 0 {
                    worst = code;
                }
            }
            Ok(Err(err)) => {
                if let Some(ref sess) = session {
                    tunnel::cleanup(&runtime, sess);
                }
                return Err(err);
            }
            Err(_) => {
                if let Some(ref sess) = session {
                    tunnel::cleanup(&runtime, sess);
                }
                return Err(Error::Message("command thread panicked".into()));
            }
        }
    }

    if let Some(ref sess) = session {
        tunnel::cleanup(&runtime, sess);
    }

    if shutting_down.load(Ordering::SeqCst) {
        return Ok(if worst == 0 { 1 } else { worst });
    }

    if config.after_commands().is_empty() {
        info!("No afterCommands to run");
    } else if worst == 0 {
        info!("Running afterCommands");
        for command in config.after_commands() {
            info!("Running afterCommand: {command}");
            run_hook(command, &exec_env, false)?;
        }
    }

    let _ = index;
    Ok(worst)
}

#[allow(clippy::too_many_arguments)]
fn spawn_one(
    index: usize,
    spec: CommandSpec,
    prefix_length: usize,
    conc_opts: &ConcurrentlyOptions,
    tunnel_enabled: bool,
    handle_input: bool,
    kill_on_failure: bool,
    failed: &Arc<AtomicBool>,
    shutting_down: &Arc<AtomicBool>,
    children: &Arc<Mutex<Vec<ChildHandle>>>,
    joins: &mut Vec<thread::JoinHandle<Result<u8, Error>>>,
) {
    let name = {
        let n = spec.display_name(prefix_length);
        if n.is_empty() {
            index.to_string()
        } else {
            n
        }
    };
    let color = conc_opts.resolve_prefix_color(spec.prefix_color.as_deref(), index);
    let failed = Arc::clone(failed);
    let shutting_down = Arc::clone(shutting_down);
    let children = Arc::clone(children);
    joins.push(thread::spawn(move || {
        run_command(
            name,
            spec,
            color,
            children,
            failed,
            shutting_down,
            kill_on_failure,
            handle_input,
            tunnel_enabled,
        )
    }));
}

struct ChildHandle {
    #[cfg(unix)]
    pgid: Option<i32>,
    child_id: u32,
}

fn run_hook(command: &str, env: &[(String, String)], before: bool) -> Result<(), Error> {
    let mut cmd = shell_command(command);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = cmd.status()?;
    if !status.success() {
        if before {
            return Err(Error::BeforeCommandFailed {
                command: command.to_string(),
                status,
            });
        }
        return Err(Error::AfterCommandFailed {
            command: command.to_string(),
            status,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_command(
    name: String,
    spec: CommandSpec,
    color: Option<String>,
    children: Arc<Mutex<Vec<ChildHandle>>>,
    failed: Arc<AtomicBool>,
    shutting_down: Arc<AtomicBool>,
    kill_on_failure: bool,
    handle_input: bool,
    tunnel_enabled: bool,
) -> Result<u8, Error> {
    let mut cmd = shell_command(&spec.command);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    for (k, v) in spec.effective_env(tunnel_enabled) {
        cmd.env(k, v);
    }
    cmd.stdin(if handle_input {
        Stdio::inherit()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn()?;
    let id = child.id();

    {
        let mut lock = children.lock().unwrap();
        lock.push(ChildHandle {
            #[cfg(unix)]
            pgid: Some(id as i32),
            child_id: id,
        });
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let prefix = name.clone();
    let color_out = color.clone();

    let t_out = stdout.map(|pipe| {
        let prefix = prefix.clone();
        let color = color_out.clone();
        thread::spawn(move || prefix_pipe(pipe, &prefix, color.as_deref(), false))
    });
    let t_err = stderr.map(|pipe| {
        let prefix = prefix.clone();
        let color = color.clone();
        thread::spawn(move || prefix_pipe(pipe, &prefix, color.as_deref(), true))
    });

    let status = child.wait()?;
    if let Some(t) = t_out {
        let _ = t.join();
    }
    if let Some(t) = t_err {
        let _ = t.join();
    }

    let code = status.code().unwrap_or(1) as u8;
    info!(
        "Command {name} {}",
        command_finish_word(status.code(), shutting_down.load(Ordering::SeqCst))
    );

    if code != 0 && !shutting_down.load(Ordering::SeqCst) {
        failed.store(true, Ordering::SeqCst);
        if kill_on_failure {
            kill_all(&children);
        }
    }

    Ok(code)
}

/// Ctrl+C / SIGTERM is a stop, not a command failure.
fn command_finish_word(exit_code: Option<i32>, shutting_down: bool) -> &'static str {
    if shutting_down || exit_code.is_none() {
        "stopped"
    } else if exit_code == Some(0) {
        "exited"
    } else {
        "errored"
    }
}

fn prefix_pipe<R: std::io::Read>(pipe: R, name: &str, color: Option<&str>, is_err: bool) {
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let painted = format_prefixed_line(name, &line, color);
        if is_err {
            eprintln!("{painted}");
        } else {
            println!("{painted}");
        }
    }
}

/// Concurrently-style line: colored `[name]` then a space then the uncolored rest.
fn format_prefixed_line(name: &str, line: &str, color: Option<&str>) -> String {
    let prefix = colorize(&format!("[{name}]"), color);
    format!("{prefix} {line}")
}

fn colorize(text: &str, color: Option<&str>) -> String {
    match color.map(|c| c.to_ascii_lowercase()) {
        Some(c) if c == "red" => text.red().to_string(),
        Some(c) if c == "green" => text.green().to_string(),
        Some(c) if c == "yellow" => text.yellow().to_string(),
        Some(c) if c == "blue" => text.blue().to_string(),
        Some(c) if c == "magenta" => text.magenta().to_string(),
        Some(c) if c == "cyan" => text.cyan().to_string(),
        Some(c) if c == "white" => text.white().to_string(),
        _ => text.to_string(),
    }
}

fn shell_command(command: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

fn kill_all(children: &Arc<Mutex<Vec<ChildHandle>>>) {
    let Ok(lock) = children.lock() else {
        return;
    };
    for child in lock.iter() {
        #[cfg(unix)]
        {
            if let Some(pgid) = child.pgid {
                unsafe {
                    libc_kill_group(pgid);
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &child.child_id.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = child.child_id;
    }
}

#[cfg(unix)]
unsafe fn libc_kill_group(pgid: i32) {
    libc_kill(-pgid.abs(), 15);
}

#[cfg(unix)]
unsafe fn libc_kill(pid: i32, sig: i32) {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    let _ = kill(pid, sig);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::EnvValue;
    use std::collections::BTreeMap;

    #[test]
    fn tunnel_env_merges_only_when_enabled() {
        let mut env = BTreeMap::new();
        env.insert("FOO".into(), EnvValue::String("base".into()));
        let mut tunnel_env = BTreeMap::new();
        tunnel_env.insert("FOO".into(), EnvValue::String("tun".into()));
        let spec = CommandSpec {
            command: "echo".into(),
            env: Some(env),
            tunnel_env: Some(tunnel_env),
            ..CommandSpec::default()
        };
        assert_eq!(spec.effective_env(false).get("FOO").unwrap(), "base");
        assert_eq!(spec.effective_env(true).get("FOO").unwrap(), "tun");
    }

    #[test]
    fn auto_prefix_cycles() {
        let opts = ConcurrentlyOptions {
            prefix_colors: Some(crate::config::types::PrefixColors::Named("auto".into())),
            ..ConcurrentlyOptions::default()
        };
        assert_eq!(opts.resolve_prefix_color(None, 0).as_deref(), Some("cyan"));
        assert_eq!(
            opts.resolve_prefix_color(Some("green"), 0).as_deref(),
            Some("green")
        );
        let off = ConcurrentlyOptions {
            prefix_colors: Some(crate::config::types::PrefixColors::Flag(false)),
            ..ConcurrentlyOptions::default()
        };
        assert_eq!(off.resolve_prefix_color(None, 0), None);
    }

    #[test]
    fn ctrl_c_is_stopped_not_errored() {
        assert_eq!(command_finish_word(None, false), "stopped");
        assert_eq!(command_finish_word(Some(1), true), "stopped");
        assert_eq!(command_finish_word(Some(0), false), "exited");
        assert_eq!(command_finish_word(Some(1), false), "errored");
    }

    #[test]
    fn prefix_is_brackets_then_space() {
        assert_eq!(format_prefixed_line("nuxt", "ready", None), "[nuxt] ready");
        assert_eq!(format_prefixed_line("tunnel", "ok", None), "[tunnel] ok");
    }

    #[test]
    fn prefix_truncates_name_inside_brackets() {
        let spec = CommandSpec {
            name: Some("verylongname".into()),
            ..CommandSpec::default()
        };
        let name = spec.display_name(10);
        assert_eq!(format_prefixed_line(&name, "x", None), "[verylongna] x");
    }

    #[test]
    fn prefix_color_paints_only_bracket_token() {
        let rest = " hello world";
        let out = format_prefixed_line("nuxt", "hello world", Some("green"));
        assert!(out.ends_with(rest), "uncolored remainder: {out:?}");
        let prefix_part = &out[..out.len() - rest.len()];
        assert!(
            prefix_part.contains("[nuxt]"),
            "colored prefix contains [nuxt]: {prefix_part:?}"
        );
        assert!(
            prefix_part.contains('\u{1b}'),
            "ANSI on prefix: {prefix_part:?}"
        );
        assert!(
            !out[out.len() - rest.len()..].contains('\u{1b}'),
            "remainder has no ANSI: {out:?}"
        );
    }

    #[test]
    fn prefix_no_color_has_no_ansi() {
        let out = format_prefixed_line("echo", "hi", None);
        assert_eq!(out, "[echo] hi");
        assert!(!out.contains('\u{1b}'));
    }
}
