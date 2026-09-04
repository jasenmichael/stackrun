use crate::config::types::{Command, ProcessOptions};
use crate::error::Error;
use crate::logging;
use std::io::{BufRead, BufReader};
use std::process::{Command as StdCommand, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

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

/// Shared child list + SIGINT flag. Install once in [`crate::stack::run`].
#[derive(Clone)]
pub struct InterruptState {
    children: Arc<Mutex<Vec<ChildHandle>>>,
    shutting_down: Arc<AtomicBool>,
}

impl InterruptState {
    pub fn new() -> Self {
        Self {
            children: Arc::new(Mutex::new(Vec::new())),
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn install_handler(&self) {
        let shutting_down = Arc::clone(&self.shutting_down);
        let children = Arc::clone(&self.children);
        let _ = ctrlc::set_handler(move || {
            shutting_down.store(true, Ordering::SeqCst);
            kill_all(&children);
        });
    }

    fn mark_stopped(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    fn register(&self, handle: ChildHandle) {
        if let Ok(mut lock) = self.children.lock() {
            lock.push(handle);
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }
}

impl Default for InterruptState {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn and join concurrent commands. Does not run hooks or tunnels.
pub fn run_concurrent(
    run: ConcurrentRun,
    interrupt: &InterruptState,
) -> Result<ConcurrentOutcome, Error> {
    let prefix_length = run.options.prefix_length_or_default();
    let kill_on_failure = run.options.kill_others_on_failure();
    let handle_input = run.options.handle_input_or_default();
    let conc_opts = run.options;
    let apply_tunnel_env = run.apply_tunnel_env;

    let failed = Arc::new(AtomicBool::new(false));

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
            interrupt,
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
        interrupted: interrupt.is_shutting_down(),
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
    interrupt: &InterruptState,
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
    let host_color = logging::host_color_enabled(Some(conc_opts));
    let failed = Arc::clone(failed);
    let interrupt = interrupt.clone();
    let default_cwd = conc_opts.cwd.clone();
    joins.push(thread::spawn(move || {
        run_command(
            name,
            spec,
            color,
            host_color,
            interrupt,
            failed,
            kill_on_failure,
            handle_input,
            tunnel_enabled,
            default_cwd,
        )
    }));
}

struct ChildHandle {
    #[cfg(unix)]
    pgid: Option<i32>,
    child_id: u32,
}

/// Sequential shell hook (`before` / `after`). Stdio inherit.
pub fn run_hook(
    command: &str,
    env: &[(String, String)],
    before: bool,
    cwd: Option<&str>,
    interrupt: &InterruptState,
) -> Result<(), Error> {
    let mut cmd = shell_command(command);
    for (k, v) in env {
        cmd.env(k, v);
    }
    if let Some(cwd) = cwd.filter(|s| !s.is_empty()) {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn()?;
    let id = child.id();
    interrupt.register(ChildHandle {
        #[cfg(unix)]
        pgid: Some(id as i32),
        child_id: id,
    });
    let status = child.wait()?;
    if interrupt.is_shutting_down() {
        if before {
            return Err(Error::Message("interrupted by signal".into()));
        }
        return Ok(());
    }
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
    host_color: bool,
    interrupt: InterruptState,
    failed: Arc<AtomicBool>,
    kill_on_failure: bool,
    handle_input: bool,
    tunnel_enabled: bool,
    default_cwd: Option<String>,
) -> Result<u8, Error> {
    let mut cmd = spawn_command(&spec);
    if let Some(cwd) = spec
        .cwd
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| default_cwd.as_deref().filter(|s| !s.is_empty()))
    {
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
    interrupt.register(ChildHandle {
        #[cfg(unix)]
        pgid: Some(id as i32),
        child_id: id,
    });

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
    let shutting = interrupt.is_shutting_down();
    logging::emit_opt(
        format!(
            "Command {name} {}",
            command_finish_word(status.code(), shutting)
        ),
        host_color,
    );

    if shutting {
        return Ok(0);
    }
    if code != 0 {
        let first_failure = !failed.swap(true, Ordering::SeqCst);
        if kill_on_failure {
            interrupt.mark_stopped();
            kill_all(&interrupt.children);
        }
        return Ok(if first_failure { code } else { 0 });
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
    let prefix = logging::colorize(&format!("[{name}]"), color);
    format!("{prefix} {line}")
}

fn spawn_command(spec: &Command) -> StdCommand {
    if let Some(argv) = spec.argv.as_ref().filter(|a| !a.is_empty()) {
        let mut cmd = StdCommand::new(&argv[0]);
        cmd.args(&argv[1..]);
        return cmd;
    }
    shell_command(&spec.run)
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

    #[test]
    fn argv_spawn_skips_shell() {
        let spec = Command {
            run: "echo ignored; true".into(),
            argv: Some(vec!["cloudflared".into(), "tunnel".into(), "--url".into()]),
            ..Command::default()
        };
        let cmd = spawn_command(&spec);
        assert_eq!(cmd.get_program(), "cloudflared");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, ["tunnel", "--url"]);
    }

    #[test]
    fn empty_argv_uses_shell() {
        let spec = Command {
            run: "echo hi".into(),
            argv: Some(Vec::new()),
            ..Command::default()
        };
        let cmd = spawn_command(&spec);
        if cfg!(windows) {
            assert_eq!(cmd.get_program(), "cmd");
        } else {
            assert_eq!(cmd.get_program(), "sh");
        }
    }
}
