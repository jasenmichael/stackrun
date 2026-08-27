//! CLI flag coverage via the `stackrun` binary and `--dry-run` JSON.

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

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
        .env_remove("RUST_LOG");
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

fn write_yaml(dir: &Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).unwrap();
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
    assert_eq!(report["config"]["tunnelEnabled"], false);
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
            r#"{"commands":[{"name":"hi","command":"echo hello","cwd":"./x"}]}"#,
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
    assert_eq!(report["config"]["commands"][0]["command"], "echo hello");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./x");
}

#[test]
fn short_c_and_long_config() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "via-short.yaml",
        "commands:\n  - name: short\n    command: echo short\n",
    );
    write_yaml(
        dir.path(),
        "via-long.yaml",
        "commands:\n  - name: long\n    command: echo long\n",
    );

    let short = run_in(dir.path(), &["--dry-run", "-c", "via-short.yaml"]);
    assert!(
        short.status.success(),
        "{}",
        String::from_utf8_lossy(&short.stderr)
    );
    let short_json = stdout_json(&short);
    assert_eq!(short_json["config"]["commands"][0]["name"], "short");
    assert!(short_json["configFile"]
        .as_str()
        .unwrap()
        .ends_with("via-short.yaml"));

    let long = run_in(dir.path(), &["--dry-run", "--config", "via-long.yaml"]);
    assert!(
        long.status.success(),
        "{}",
        String::from_utf8_lossy(&long.stderr)
    );
    let long_json = stdout_json(&long);
    assert_eq!(long_json["config"]["commands"][0]["name"], "long");
    assert!(long_json["configFile"]
        .as_str()
        .unwrap()
        .ends_with("via-long.yaml"));
}

#[test]
fn positional_config() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "custom.yaml",
        "commands:\n  - name: pos\n    command: echo positional\n    cwd: ./pos\n",
    );
    let output = run_in(dir.path(), &["--dry-run", "custom.yaml"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_eq!(report["config"]["commands"][0]["name"], "pos");
    assert_eq!(
        report["config"]["commands"][0]["command"],
        "echo positional"
    );
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("custom.yaml"));
}

#[test]
fn short_c_wins_over_positional() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "a.yaml",
        "commands:\n  - name: from-c\n    command: echo a\n",
    );
    write_yaml(
        dir.path(),
        "b.yaml",
        "commands:\n  - name: from-pos\n    command: echo b\n",
    );
    let output = run_in(dir.path(), &["--dry-run", "-c", "a.yaml", "b.yaml"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_eq!(report["config"]["commands"][0]["name"], "from-c");
    assert!(report["configFile"].as_str().unwrap().ends_with("a.yaml"));
}

#[test]
fn json_overlay_wins_over_file() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "stack.config.yaml",
        "tunnelEnabled: false\ncommands:\n  - name: file\n    command: echo file\n",
    );
    let output = run_in(
        dir.path(),
        &[
            "--dry-run",
            "--json",
            r#"{"tunnelEnabled":true,"commands":[{"name":"json","command":"echo json"}]}"#,
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_eq!(report["config"]["tunnelEnabled"], true);
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
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "stack.config.yaml",
        "commands:\n  - name: file\n    command: echo file\n",
    );
    let output = run_in(dir.path(), &["--dry-run", "--command", "python server.py"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    let cmds = report["config"]["commands"].as_array().unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0], "python server.py");
}

#[test]
fn tunnel_short_and_long() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "stack.config.yaml",
        "commands:\n  - command: echo x\n",
    );
    for flag in ["-t", "--tunnel"] {
        let output = run_in(dir.path(), &["--dry-run", flag]);
        assert!(
            output.status.success(),
            "{flag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = stdout_json(&output);
        assert_eq!(report["config"]["tunnelEnabled"], true, "{flag}");
    }
}

#[test]
fn tunnel_env_true_only() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "stack.config.yaml",
        "commands:\n  - command: echo x\n",
    );

    let on = run_in_with_env(dir.path(), &["--dry-run"], &[("TUNNEL", Some("true"))]);
    assert!(
        on.status.success(),
        "{}",
        String::from_utf8_lossy(&on.stderr)
    );
    assert_eq!(stdout_json(&on)["config"]["tunnelEnabled"], true);

    let off = run_in_with_env(dir.path(), &["--dry-run"], &[("TUNNEL", Some("1"))]);
    assert!(
        off.status.success(),
        "{}",
        String::from_utf8_lossy(&off.stderr)
    );
    assert_eq!(stdout_json(&off)["config"]["tunnelEnabled"], false);
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
            r#"{"commands":[{"command":"echo x","url":"http://localhost:1","tunnelUrl":"https://x.example"}]}"#,
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_eq!(report["config"]["tunnelEnabled"], true);
    // --command replaces commands after json overlay
    assert_eq!(report["config"]["commands"][0], "echo x");
}

#[test]
fn dry_run_does_not_spawn_hooks_or_commands() {
    let dir = tempdir().unwrap();
    let before = dir.path().join("before.ran");
    let child = dir.path().join("child.ran");
    write_yaml(
        dir.path(),
        "stack.config.yaml",
        &format!(
            "beforeCommands:\n  - touch {}\nafterCommands:\n  - touch {}\ncommands:\n  - name: child\n    command: touch {}\n",
            before.display(),
            dir.path().join("after.ran").display(),
            child.display()
        ),
    );
    let output = run_in(dir.path(), &["--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert!(report["config"]["beforeCommands"][0]
        .as_str()
        .unwrap()
        .contains("before.ran"));
    assert_eq!(report["config"]["commands"][0]["name"], "child");
    assert!(!before.exists());
    assert!(!child.exists());
    assert!(!dir.path().join("after.ran").exists());
}

#[test]
fn dry_run_redacts_cf_token_and_omits_env_token() {
    let dir = tempdir().unwrap();
    write_yaml(
        dir.path(),
        "stack.config.yaml",
        "cfTunnelConfig:\n  cfToken: file-secret-token\n  tunnelName: demo\ncommands:\n  - command: echo x\n",
    );
    let output = run_in_with_env(
        dir.path(),
        &["--dry-run"],
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
    let report = stdout_json(&output);
    assert_eq!(report["config"]["cfTunnelConfig"]["cfToken"], "[redacted]");
    assert_eq!(report["config"]["cfTunnelConfig"]["tunnelName"], "demo");
}
