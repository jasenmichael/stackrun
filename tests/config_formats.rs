//! Load every supported config format and assert `--dry-run` JSON.

use serde_json::Value;
use stackrun::config::load::{format_dry_run, load_config, LoadOptions};
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

fn assert_cmd(report: &Value, name: &str, command: &str) {
    let cmds = report["config"]["commands"].as_array().expect("commands");
    let entry = cmds
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("missing command {name}: {cmds:?}"));
    assert_eq!(entry["command"], command, "{name}");
}

fn jiti_available() -> bool {
    let Ok(status) = Command::new("node")
        .args(["--input-type=module", "-e", "await import('jiti')"])
        .status()
    else {
        return false;
    };
    status.success()
}

#[test]
fn loads_json() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.json"),
        r#"{
  "tunnelEnabled": false,
  "commands": [{
    "name": "json",
    "command": "echo from-json",
    "cwd": "./json-cwd",
    "env": { "FROM": "json" },
    "prefixColor": "green"
  }]
}"#,
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("stack.config.json"));
    assert_cmd(&report, "json", "echo from-json");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./json-cwd");
    assert_eq!(report["config"]["commands"][0]["env"]["FROM"], "json");
    assert_eq!(report["config"]["commands"][0]["prefixColor"], "green");
    assert_eq!(report["config"]["tunnelEnabled"], false);
}

#[test]
fn loads_jsonc() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.jsonc"),
        r#"{
  // comment
  "commands": [{
    "name": "jsonc",
    "command": "echo from-jsonc",
    "cwd": "./jsonc-cwd",
  }],
}"#,
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("stack.config.jsonc"));
    assert_cmd(&report, "jsonc", "echo from-jsonc");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./jsonc-cwd");
}

#[test]
fn loads_json5() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.json5"),
        "{
  commands: [{
    name: 'json5',
    command: 'echo from-json5',
    cwd: './json5-cwd',
  }],
}",
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("stack.config.json5"));
    assert_cmd(&report, "json5", "echo from-json5");
}

#[test]
fn loads_yaml() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        r#"
tunnelEnabled: true
beforeCommands:
  - echo before-yaml
afterCommands:
  - echo after-yaml
commands:
  - name: yaml
    command: echo from-yaml
    cwd: ./yaml-cwd
    env:
      FROM: yaml
    url: http://localhost:4000
    tunnelUrl: https://yaml.example
    tunnelEnv:
      TUNNELED: "1"
"#,
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("stack.config.yaml"));
    assert_eq!(report["config"]["tunnelEnabled"], true);
    assert_eq!(report["config"]["beforeCommands"][0], "echo before-yaml");
    assert_eq!(report["config"]["afterCommands"][0], "echo after-yaml");
    assert_cmd(&report, "yaml", "echo from-yaml");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./yaml-cwd");
    assert_eq!(report["config"]["commands"][0]["env"]["FROM"], "yaml");
    assert_eq!(
        report["config"]["commands"][0]["url"],
        "http://localhost:4000"
    );
    assert_eq!(
        report["config"]["commands"][0]["tunnelUrl"],
        "https://yaml.example"
    );
    assert_eq!(
        report["config"]["commands"][0]["tunnelEnv"]["TUNNELED"],
        "1"
    );
}

#[test]
fn loads_yml() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yml"),
        "commands:\n  - name: yml\n    command: echo from-yml\n",
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("stack.config.yml"));
    assert_cmd(&report, "yml", "echo from-yml");
}

#[test]
fn loads_toml() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.toml"),
        r#"
tunnelEnabled = false

[[commands]]
name = "toml"
command = "echo from-toml"
cwd = "./toml-cwd"
"#,
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with("stack.config.toml"));
    assert_cmd(&report, "toml", "echo from-toml");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./toml-cwd");
    assert_eq!(report["config"]["tunnelEnabled"], false);
}

#[test]
fn discovers_config_dir_stack_yaml() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join(".config")).unwrap();
    fs::write(
        dir.path().join(".config/stack.yaml"),
        r#"
commands:
  - name: hidden
    command: echo from-config-dir
    cwd: ./hidden
"#,
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with(".config/stack.yaml"));
    assert_cmd(&report, "hidden", "echo from-config-dir");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./hidden");
}

#[test]
fn cwd_stackrc_merges() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "tunnelEnabled: true\ncommands:\n  - name: main\n    command: echo main\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".stackrc"),
        "tunnelEnabled=false\nbeforeCommands[]=echo rc\n",
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert_eq!(report["config"]["tunnelEnabled"], true);
    assert_eq!(report["config"]["beforeCommands"][0], "echo rc");
    assert_cmd(&report, "main", "echo main");
}

#[test]
fn dotenv_loaded_without_dumping_env_into_dry_run() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join(".env"),
        "BASE=/tmp/stackrun-dry\nAPP_DIR=${BASE}/app\nDOTENV_SECRET=env-file-secret\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - name: env\n    command: echo from-env\n    cwd: ./env-cwd\n",
    )
    .unwrap();
    let output = run_in(dir.path(), &["--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("env-file-secret"), "{stdout}");
    let report = stdout_json(&output);
    assert_cmd(&report, "env", "echo from-env");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./env-cwd");
}

#[test]
fn local_extends() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("base.yaml"),
        "beforeCommands: [echo base]\ncommands:\n  - name: base\n    command: echo basecmd\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "extends: ./base.yaml\ncommands:\n  - name: child\n    command: echo child\n",
    )
    .unwrap();
    let report = dry_run_ok(dir.path(), &["--dry-run"]);
    assert_eq!(report["config"]["beforeCommands"][0], "echo base");
    let cmds = report["config"]["commands"].as_array().unwrap();
    assert_eq!(cmds[0]["name"], "child");
    assert_eq!(cmds[0]["command"], "echo child");
    assert_eq!(cmds[1]["name"], "base");
    assert_eq!(cmds[1]["command"], "echo basecmd");
}

#[test]
fn node_env_overlay() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        r#"
tunnelEnabled: false
$development:
  tunnelEnabled: true
commands:
  - name: overlay
    command: echo overlay
"#,
    )
    .unwrap();
    let output = run_in_with_env(
        dir.path(),
        &["--dry-run"],
        &[("NODE_ENV", Some("development"))],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = stdout_json(&output);
    assert_eq!(report["config"]["tunnelEnabled"], true);
    assert_cmd(&report, "overlay", "echo overlay");
}

fn dry_run_lib(cwd: &Path) -> Value {
    let loaded = load_config(LoadOptions::for_cwd(cwd)).expect("load_config");
    let json = format_dry_run(&loaded).expect("format_dry_run");
    serde_json::from_str(&json).expect("dry-run JSON")
}

fn write_js_config(dir: &Path, filename: &str, name: &str, command: &str) {
    let body = format!(
        "export default {{ commands: [{{ name: \"{name}\", command: \"{command}\", cwd: \"./{name}\" }}] }};\n"
    );
    fs::write(dir.join(filename), body).unwrap();
}

fn write_cjs_config(dir: &Path, filename: &str, name: &str, command: &str) {
    let body = format!(
        "module.exports = {{ commands: [{{ name: \"{name}\", command: \"{command}\", cwd: \"./{name}\" }}] }};\n"
    );
    fs::write(dir.join(filename), body).unwrap();
}

fn skip_jiti() -> bool {
    if jiti_available() {
        false
    } else {
        eprintln!("skip: node + jiti not available");
        true
    }
}

fn assert_jiti_format(filename: &str, name: &str, command: &str, cjs: bool) {
    if skip_jiti() {
        return;
    }
    let dir = tempdir().unwrap();
    if cjs {
        write_cjs_config(dir.path(), filename, name, command);
    } else {
        write_js_config(dir.path(), filename, name, command);
    }
    let report = dry_run_lib(dir.path());
    assert!(
        report["configFile"].as_str().unwrap().ends_with(filename),
        "{}",
        report["configFile"]
    );
    assert_cmd(&report, name, command);
    assert_eq!(report["config"]["commands"][0]["cwd"], format!("./{name}"));
}

#[test]
fn loads_js_via_jiti() {
    assert_jiti_format("stack.config.js", "js", "echo from-js", false);
}

#[test]
fn loads_ts_via_jiti() {
    assert_jiti_format("stack.config.ts", "ts", "echo from-ts", false);
}

#[test]
fn loads_mjs_via_jiti() {
    assert_jiti_format("stack.config.mjs", "mjs", "echo from-mjs", false);
}

#[test]
fn loads_cjs_via_jiti() {
    assert_jiti_format("stack.config.cjs", "cjs", "echo from-cjs", true);
}

#[test]
fn loads_mts_via_jiti() {
    assert_jiti_format("stack.config.mts", "mts", "echo from-mts", false);
}

#[test]
fn loads_cts_via_jiti() {
    assert_jiti_format("stack.config.cts", "cts", "echo from-cts", true);
}

#[test]
fn jiti_config_sees_interpolated_dotenv() {
    if skip_jiti() {
        return;
    }
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join(".env"),
        "BASE=/tmp/stackrun-dry\nAPP_DIR=${BASE}/app\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("stack.config.js"),
        r#"export default { commands: [{ name: "dotenv", command: process.env.APP_DIR, cwd: process.env.APP_DIR }] };
"#,
    )
    .unwrap();
    std::env::remove_var("APP_DIR");
    std::env::remove_var("BASE");
    let report = dry_run_lib(dir.path());
    std::env::remove_var("APP_DIR");
    std::env::remove_var("BASE");
    assert_cmd(&report, "dotenv", "/tmp/stackrun-dry/app");
    assert_eq!(
        report["config"]["commands"][0]["cwd"],
        "/tmp/stackrun-dry/app"
    );
}
