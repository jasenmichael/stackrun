//! Docker Compose fixtures: `before` up, dummy probe, `after` down.
//!
//! Live tests skip when `docker` / `docker compose` is missing or `docker info` fails
//! (no daemon). Spawn the `stackrun` binary with `cwd` = fixture dir so compose files
//! resolve. `stack::run` uses the process cwd for hooks; cargo test is parallel, so
//! we do not `chdir`.
//!
//! Product skips `after` when any concurrent command fails (no `finally`).
//! Ctrl+C is the other stop: every command dies, then `after` run.
//! These fixtures still `down -v` on the success path. A [`ComposeCleanup`] guard also
//! downs if the dummy fails or the test panics, so leftover containers do not linger.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/docker_stack")
}

fn fixture(name: &str) -> PathBuf {
    fixtures_root().join(name)
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_stackrun"))
}

fn strip_run_env(cmd: &mut Command) {
    cmd.env_remove("TUNNEL")
        .env_remove("NODE_ENV")
        .env_remove("CF_TOKEN")
        .env_remove("CLOUDFLARE_TOKEN")
        .env_remove("CLOUDFLARE_TUNNEL_NAME")
        .env_remove("CF_TUNNEL_NAME")
        .env_remove("RUST_LOG");
}

fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

fn docker_skip_reason() -> Option<&'static str> {
    static REASON: OnceLock<Option<&'static str>> = OnceLock::new();
    *REASON.get_or_init(|| {
        if !which("docker") {
            return Some("docker not on PATH");
        }
        let compose = Command::new("docker")
            .args(["compose", "version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(compose, Ok(s) if s.success()) {
            return Some("docker compose (v2) not available");
        }
        let info = Command::new("docker")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(info, Ok(s) if s.success()) {
            return Some("docker daemon not reachable (docker info failed)");
        }
        None
    })
}

fn skip_docker() -> bool {
    if let Some(reason) = docker_skip_reason() {
        eprintln!("skip: {reason}");
        true
    } else {
        false
    }
}

/// Tear down the fixture compose project even when stackrun skips `after`.
struct ComposeCleanup {
    dir: PathBuf,
}

impl ComposeCleanup {
    fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }
}

impl Drop for ComposeCleanup {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["compose", "-f", "docker-compose.yml", "down", "-v"])
            .current_dir(&self.dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn run_stackrun(cwd: &Path, timeout: Duration) -> Output {
    let mut cmd = bin();
    strip_run_env(&mut cmd);
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().expect("spawn stackrun");
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => panic!("wait stackrun: {err}"),
        Err(_) => {
            kill_stackrun(pid);
            let _ = rx.recv_timeout(Duration::from_secs(5));
            panic!("stackrun hung after {timeout:?} (cwd {})", cwd.display());
        }
    }
}

fn kill_stackrun(pid: u32) {
    #[cfg(unix)]
    unsafe {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        let pgid = pid as i32;
        kill(-pgid, 15);
        thread::sleep(Duration::from_millis(200));
        kill(-pgid, 9);
    }
    #[cfg(not(unix))]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn compose_ps_ids(dir: &Path) -> String {
    let output = Command::new("docker")
        .args(["compose", "-f", "docker-compose.yml", "ps", "-q"])
        .current_dir(dir)
        .output()
        .expect("docker compose ps");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stdout_json(output: &Output) -> Value {
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|err| {
        panic!(
            "stdout not JSON ({err}): {text:?}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn dry_run(cwd: &Path) -> Value {
    let mut cmd = bin();
    strip_run_env(&mut cmd);
    let output = cmd
        .current_dir(cwd)
        .args(["--dry-run"])
        .output()
        .expect("spawn stackrun --dry-run");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    stdout_json(&output)
}

fn assert_success(output: &Output, cwd: &Path) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stackrun failed in {}\nstatus {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        cwd.display(),
        output.status.code()
    );
}

const LIVE_TIMEOUT: Duration = Duration::from_secs(90);

#[test]
fn fixtures_dry_run_without_docker() {
    for name in ["redis", "web", "multi"] {
        let report = dry_run(&fixture(name));
        assert_eq!(
            report["config"]["tunnel"], false,
            "{name}: tunnels must stay off"
        );
        assert_eq!(report["config"]["process"]["handleInput"], false);
        assert_eq!(report["config"]["process"]["killOthers"], "failure");
        let before = report["config"]["before"][0].as_str().unwrap_or("");
        assert!(
            before.contains("docker compose") && before.contains("up"),
            "{name} before: {before}"
        );
        let after = report["config"]["after"][0].as_str().unwrap_or("");
        assert!(
            after.contains("docker compose") && after.contains("down"),
            "{name} after: {after}"
        );
        let cmds = report["config"]["commands"].as_array().expect("commands");
        for cmd in cmds {
            assert!(
                cmd.get("tunnel").is_none() || cmd["tunnel"].is_null(),
                "{name} must not set tunnel: {cmd}"
            );
        }
    }
}

#[test]
fn redis_stack_up_probe_down() {
    if skip_docker() {
        return;
    }
    let dir = fixture("redis");
    let _guard = ComposeCleanup::new(&dir);
    let output = run_stackrun(&dir, LIVE_TIMEOUT);
    assert_success(&output, &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[redis]") && stdout.contains("PONG"),
        "expected [redis] PONG, got:\n{stdout}"
    );
    assert!(
        compose_ps_ids(&dir).is_empty(),
        "afterCommands should have torn down stackrun-test-redis"
    );
}

#[test]
fn web_stack_up_probe_down() {
    if skip_docker() {
        return;
    }
    let dir = fixture("web");
    let _guard = ComposeCleanup::new(&dir);
    let output = run_stackrun(&dir, LIVE_TIMEOUT);
    assert_success(&output, &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[web]"),
        "expected [web] prefix, got:\n{stdout}"
    );
    assert!(
        stdout.to_ascii_lowercase().contains("nginx"),
        "expected nginx welcome body, got:\n{stdout}"
    );
    assert!(
        compose_ps_ids(&dir).is_empty(),
        "afterCommands should have torn down stackrun-test-web"
    );
}

#[test]
fn multi_stack_up_probe_down() {
    if skip_docker() {
        return;
    }
    let dir = fixture("multi");
    let _guard = ComposeCleanup::new(&dir);
    let output = run_stackrun(&dir, LIVE_TIMEOUT);
    assert_success(&output, &dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[redis]") && stdout.contains("PONG"),
        "expected [redis] PONG, got:\n{stdout}"
    );
    assert!(
        stdout.contains("[web]") && stdout.to_ascii_lowercase().contains("nginx"),
        "expected [web] nginx body, got:\n{stdout}"
    );
    assert!(
        compose_ps_ids(&dir).is_empty(),
        "afterCommands should have torn down stackrun-test-multi"
    );
}
