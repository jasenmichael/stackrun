use crate::config::types::{Command, ProcessOptions};
use crate::error::Error;
use owo_colors::OwoColorize;
use std::io::{BufRead, BufReader};
use std::process::{Command as StdCommand, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::info;

/// Concurrent children to spawn: prefixed output, kill-others, SIGINT.
pub struct ConcurrentRun {
    pub commands: Vec<Command>,
    pub options: ProcessOptions,
    pub apply_tunnel_env: bool,
}

/// Result of [`run_concurrent`] after all children have exited.
pub struct ConcurrentOutcome {
    pub worst_code: u8,
    pub interrupted: bool,
}

/// Spawn and join concurrent commands. Does not run hooks or tunnels.
pub fn run_concurrent(run: ConcurrentRun) -> Result<ConcurrentOutcome, Error> {
    let prefix_length = run.options.prefix_length_or_default();
    let kill_on_failure = run.options.kill_others_on_failure();
    let handle_input = run.options.handle_input_or_default();
    let conc_opts = run.options;
    let apply_tunnel_env = run.apply_tunnel_env;

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
    for (index, spec) in run.commands.into_iter().enumerate() {
        spawn_one(
            index,
            spec,
            prefix_length,
            &conc_opts,
            apply_tunnel_env,
            handle_input,
            kill_on_failure,
            &failed,
            &shutting_down,
            &children,
            &mut joins,
        );
    }

    let mut worst: u8 = 0;
    for join in joins {
        match join.join() {
            Ok(Ok(code)) => {
                if code != 0 && worst == 0 {
                    worst = code;
                }
            }
            Ok(Err(err)) => return Err(err),
            Err(_) => return Err(Error::Message("command thread panicked".into())),
        }
    }

    Ok(ConcurrentOutcome {
        worst_code: worst,
        interrupted: shutting_down.load(Ordering::SeqCst),
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_one(
    index: usize,
    spec: Command,
    prefix_length: usize,
    conc_opts: &ProcessOptions,
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
    let color = conc_opts.resolve_prefix_color(spec.color.as_deref(), index);
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

/// Sequential shell hook (`before` / `after`). Stdio inherit.
pub fn run_hook(command: &str, env: &[(String, String)], before: bool) -> Result<(), Error> {
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
    spec: Command,
    color: Option<String>,
    children: Arc<Mutex<Vec<ChildHandle>>>,
    failed: Arc<AtomicBool>,
    shutting_down: Arc<AtomicBool>,
    kill_on_failure: bool,
    handle_input: bool,
    tunnel_enabled: bool,
) -> Result<u8, Error> {
    let mut cmd = shell_command(&spec.run);
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

fn shell_command(command: &str) -> StdCommand {
    if cfg!(windows) {
        let mut cmd = StdCommand::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = StdCommand::new("sh");
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
            let _ = StdCommand::new("taskkill")
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
        let spec = Command {
            run: "echo".into(),
            env: Some(env),
            tunnel: Some(crate::config::types::CommandTunnel {
                env: Some(tunnel_env),
                ..crate::config::types::CommandTunnel::default()
            }),
            ..Command::default()
        };
        assert_eq!(spec.effective_env(false).get("FOO").unwrap(), "base");
        assert_eq!(spec.effective_env(true).get("FOO").unwrap(), "tun");
    }

    #[test]
    fn auto_prefix_cycles() {
        let opts = ProcessOptions {
            colors: Some(crate::config::types::PrefixColors::Named("auto".into())),
            ..ProcessOptions::default()
        };
        assert_eq!(opts.resolve_prefix_color(None, 0).as_deref(), Some("cyan"));
        assert_eq!(
            opts.resolve_prefix_color(Some("green"), 0).as_deref(),
            Some("green")
        );
        let off = ProcessOptions {
            colors: Some(crate::config::types::PrefixColors::Flag(false)),
            ..ProcessOptions::default()
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
        let spec = Command {
            name: Some("verylongname".into()),
            ..Command::default()
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
