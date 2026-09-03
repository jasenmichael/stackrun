//! CLI flag coverage via the `stackrun` binary and `--dry-run` JSON.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::tempdir;

fn flags_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cli_flags")
}

fn fixture(name: &str) -> PathBuf {
    flags_root().join(name)
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_stackrun"))
}

fn run_in(cwd: &Path, args: &[&str]) -> Output {
    run_in_with_env(cwd, args, &[])
}

fn run_in_with_env(cwd: &Path, args: &[&str], extra_env: &[(&str, Option<&str>)]) -> Output {
    let mut cmd = bin();
    cmd.current_dir(cwd)
        .env_remove("TUNNEL")
        .env_remove("NODE_ENV")
        .env_remove("CF_TOKEN")
        .env_remove("CLOUDFLARE_TOKEN")
        .env_remove("CLOUDFLARE_TUNNEL_NAME")
        .env_remove("CF_TUNNEL_NAME")
        .env_remove("RUST_LOG")
        .env_remove("STACKRUN_JITI");
    for (key, value) in extra_env {
        match value {
            Some(v) => {
                cmd.env(key, v);
            }
            None => {
                cmd.env_remove(key);
            }
        }
    }
    cmd.args(args).output().expect("spawn stackrun")
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

fn dry_run_ok(cwd: &Path, args: &[&str]) -> Value {
    let output = run_in(cwd, args);
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    stdout_json(&output)
}

#[test]
fn help_short_and_long() {
    let dir = tempdir().unwrap();
    for flag in ["-h", "--help"] {
        let output = run_in(dir.path(), &[flag]);
        assert!(output.status.success(), "{flag} failed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage:"), "{flag}: {stdout}");
        assert!(stdout.contains("--config"), "{flag}: {stdout}");
        assert!(stdout.contains("--command"), "{flag}: {stdout}");
        assert!(stdout.contains("--json"), "{flag}: {stdout}");
        assert!(stdout.contains("--tunnel"), "{flag}: {stdout}");
        assert!(stdout.contains("--dry-run"), "{flag}: {stdout}");
        assert!(stdout.contains("--jiti"), "{flag}: {stdout}");
        assert!(stdout.contains("-V"), "{flag}: {stdout}");
    }
}

#[test]
fn version_short_and_long() {
    let dir = tempdir().unwrap();
    let expected = env!("CARGO_PKG_VERSION");
    for flag in ["-V", "--version"] {
        let output = run_in(dir.path(), &[flag]);
        assert!(output.status.success(), "{flag} failed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), expected, "{flag}");
    }
}

#[test]
fn missing_config_exits_1() {
    let dir = tempdir().unwrap();
    let output = run_in(dir.path(), &["--dry-run"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No valid configuration found at stack.config"),
        "{stderr}"
    );
}

#[test]
fn dry_run_command_without_file() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("should-not-exist");
    let output = run_in(
        dir.path(),
        &[
            "--dry-run",
            "--command",
            &format!("touch {}", marker.display()),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert!(report["configFile"].is_null());
    assert_eq!(
        report["config"]["commands"][0],
        "touch ".to_string() + &marker.display().to_string()
    );
    assert_eq!(report["config"]["tunnel"], false);
    assert!(!marker.exists(), "dry-run must not spawn --command");
}

#[test]
fn dry_run_json_without_file() {
    let dir = tempdir().unwrap();
    let output = run_in(
        dir.path(),
        &[
            "--dry-run",
            "--json",
            r#"{"commands":[{"name":"hi","run":"echo hello","cwd":"./x"}]}"#,
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert!(report["configFile"].is_null());
    assert_eq!(report["config"]["commands"][0]["name"], "hi");
    assert_eq!(report["config"]["commands"][0]["run"], "echo hello");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./x");
}

#[test]
fn short_c_and_long_config() {
    let root = flags_root();
    let short = dry_run_ok(&root, &["--dry-run", "-c", "via-short.yaml"]);
    assert_eq!(short["config"]["commands"][0]["name"], "short");
    assert!(short["configFile"]
        .as_str()
        .unwrap()
        .ends_with("via-short.yaml"));

    let long = dry_run_ok(&root, &["--dry-run", "--config", "via-long.yaml"]);
    assert_eq!(long["config"]["commands"][0]["name"], "long");
    assert!(long["configFile"]
        .as_str()
        .unwrap()
        .ends_with("via-long.yaml"));
}

#[test]
fn positional_config() {
    let report = dry_run_ok(&flags_root(), &["--dry-run", "custom.yaml"]);
    assert_eq!(report["config"]["commands"][0]["name"], "pos");
    assert_eq!(report["config"]["commands"][0]["run"], "echo positional");
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("custom.yaml"));
}

#[test]
fn short_c_wins_over_positional() {
    let report = dry_run_ok(&flags_root(), &["--dry-run", "-c", "a.yaml", "b.yaml"]);
    assert_eq!(report["config"]["commands"][0]["name"], "from-c");
    assert!(report["configFile"].as_str().unwrap().ends_with("a.yaml"));
}

#[test]
fn json_overlay_wins_over_file() {
    let report = dry_run_ok(
        &flags_root(),
        &[
            "--dry-run",
            "--config",
            "overlay.yaml",
            "--json",
            r#"{"tunnel":true,"commands":[{"name":"json","run":"echo json"}]}"#,
        ],
    );
    assert_ne!(report["config"]["tunnel"], false);
    // json overlay is preferred; file commands still merge (defu concat unique)
    let names: Vec<&str> = report["config"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names[0], "json");
    assert!(names.contains(&"file"));
}

#[test]
fn command_replaces_commands() {
    let report = dry_run_ok(
        &flags_root(),
        &[
            "--dry-run",
            "--config",
            "overlay.yaml",
            "--command",
            "python server.py",
        ],
    );
    let cmds = report["config"]["commands"].as_array().unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], "python server.py");
}

#[test]
fn tunnel_short_and_long() {
    let root = flags_root();
    for flag in ["-t", "--tunnel"] {
        let report = dry_run_ok(&root, &["--dry-run", "--config", "tunnel.yaml", flag]);
        assert_ne!(report["config"]["tunnel"], false, "{flag}");
    }
}

#[test]
fn tunnel_env_true_only() {
    let root = flags_root();
    let on = run_in_with_env(
        &root,
        &["--dry-run", "--config", "tunnel.yaml"],
        &[("TUNNEL", Some("true"))],
    );
    assert!(
        on.status.success(),
        "{}",
        String::from_utf8_lossy(&on.stderr)
    );
    assert_ne!(stdout_json(&on)["config"]["tunnel"], false);

    let off = run_in_with_env(
        &root,
        &["--dry-run", "--config", "tunnel.yaml"],
        &[("TUNNEL", Some("1"))],
    );
    assert!(
        off.status.success(),
        "{}",
        String::from_utf8_lossy(&off.stderr)
    );
    assert_eq!(stdout_json(&off)["config"]["tunnel"], false);
}

#[test]
fn tunnel_with_json_and_command() {
    let dir = tempdir().unwrap();
    let output = run_in(
        dir.path(),
        &[
            "--dry-run",
            "--tunnel",
            "--command",
            "echo x",
            "--json",
            r#"{"commands":[{"run":"echo x","tunnel":{"local":"http://localhost:1","public":"https://x.example"}}]}"#,
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_ne!(report["config"]["tunnel"], false);
    // --command replaces commands after json overlay
    assert_eq!(report["config"]["commands"][0], "echo x");
}

#[test]
fn dry_run_does_not_spawn_hooks_or_commands() {
    let dir = tempdir().unwrap();
    let config = fixture("hooks.yaml");
    let config_str = config.to_string_lossy();
    let report = dry_run_ok(dir.path(), &["--dry-run", "--config", config_str.as_ref()]);
    assert!(report["config"]["before"][0]
        .as_str()
        .unwrap()
        .contains("before.ran"));
    assert_eq!(report["config"]["commands"][0]["name"], "child");
    assert!(!dir.path().join("before.ran").exists());
    assert!(!dir.path().join("child.ran").exists());
    assert!(!dir.path().join("after.ran").exists());
}

#[test]
fn dry_run_omits_process_env_secrets() {
    let output = run_in_with_env(
        &flags_root(),
        &["--dry-run", "--config", "redact.yaml"],
        &[("CF_TOKEN", Some("env-only-secret"))],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("file-secret-token"), "{stdout}");
    assert!(!stdout.contains("env-only-secret"), "{stdout}");
    let _report = stdout_json(&output);
}
