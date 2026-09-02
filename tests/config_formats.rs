//! Load every supported config format from checked-in fixtures and assert `--dry-run` JSON.

use serde_json::Value;
use stackrun::config::{format_dry_run, load_config, LoadOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn formats_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/config_formats")
}

fn fixture(subdir: &str) -> PathBuf {
    formats_root().join(subdir)
}

fn fixture_file(subdir: &str, filename: &str) -> PathBuf {
    fixture(subdir).join(filename)
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

fn dry_run_file(path: &Path) -> Value {
    let cwd = path.parent().unwrap_or(path);
    let path_str = path.to_string_lossy();
    dry_run_ok(cwd, &["--dry-run", "--config", path_str.as_ref()])
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

fn skip_jiti() -> bool {
    if jiti_available() {
        false
    } else {
        eprintln!("skip: node + jiti not available");
        true
    }
}

fn dry_run_lib(cwd: &Path) -> Value {
    let loaded = load_config(LoadOptions::for_cwd(cwd)).expect("load_config");
    let json = format_dry_run(&loaded).expect("format_dry_run");
    serde_json::from_str(&json).expect("dry-run JSON")
}

fn assert_native_format(subdir: &str, filename: &str, name: &str, command: &str) -> Value {
    let path = fixture_file(subdir, filename);
    let report = dry_run_file(&path);
    assert!(
        report["configFile"].as_str().unwrap().ends_with(filename),
        "{}",
        report["configFile"]
    );
    assert_cmd(&report, name, command);
    report
}

fn assert_jiti_format(subdir: &str, filename: &str, name: &str, command: &str) {
    if skip_jiti() {
        return;
    }
    let report = dry_run_lib(&fixture(subdir));
    assert!(
        report["configFile"].as_str().unwrap().ends_with(filename),
        "{}",
        report["configFile"]
    );
    assert_cmd(&report, name, command);
    assert_eq!(report["config"]["commands"][0]["cwd"], format!("./{name}"));
}

#[test]
fn loads_json() {
    let report = assert_native_format("json", "stack.config.json", "from-json", "echo from-json");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./json-cwd");
    assert_eq!(report["config"]["commands"][0]["env"]["FROM"], "json");
    assert_eq!(report["config"]["commands"][0]["prefixColor"], "green");
    assert_eq!(report["config"]["tunnelEnabled"], false);
}

#[test]
fn loads_jsonc() {
    let report = assert_native_format(
        "jsonc",
        "stack.config.jsonc",
        "from-jsonc",
        "echo from-jsonc",
    );
    assert_eq!(report["config"]["commands"][0]["cwd"], "./jsonc-cwd");
}

#[test]
fn loads_json5() {
    assert_native_format(
        "json5",
        "stack.config.json5",
        "from-json5",
        "echo from-json5",
    );
}

#[test]
fn loads_yaml() {
    let report = assert_native_format("yaml", "stack.config.yaml", "from-yaml", "echo from-yaml");
    assert_eq!(report["config"]["tunnelEnabled"], true);
    assert_eq!(report["config"]["beforeCommands"][0], "echo before-yaml");
    assert_eq!(report["config"]["afterCommands"][0], "echo after-yaml");
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
fn omitted_tunnel_enabled_with_ingress_enables() {
    let report = assert_native_format(
        "auto_tunnel",
        "stack.config.yaml",
        "from-auto-tunnel",
        "echo from-auto-tunnel",
    );
    assert_eq!(report["config"]["tunnelEnabled"], true);
}

#[test]
fn explicit_tunnel_enabled_false_keeps_ingress_disabled() {
    let report = assert_native_format(
        "explicit_off",
        "stack.config.yaml",
        "from-explicit-off",
        "echo from-explicit-off",
    );
    assert_eq!(report["config"]["tunnelEnabled"], false);
}

#[test]
fn loads_yml() {
    assert_native_format("yml", "stack.config.yml", "from-yml", "echo from-yml");
}

#[test]
fn loads_toml() {
    let report = assert_native_format("toml", "stack.config.toml", "from-toml", "echo from-toml");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./toml-cwd");
    assert_eq!(report["config"]["tunnelEnabled"], false);
}

#[test]
fn discovers_config_dir_stack_yaml() {
    let report = dry_run_ok(&fixture("discovery"), &["--dry-run"]);
    assert!(report["configFile"]
        .as_str()
        .unwrap()
        .ends_with(".config/stack.yaml"));
    assert_cmd(&report, "from-config-dir", "echo from-config-dir");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./hidden");
}

#[test]
fn cwd_stackrc_merges() {
    let report = dry_run_ok(&fixture("rc"), &["--dry-run"]);
    assert_eq!(report["config"]["tunnelEnabled"], true);
    assert_eq!(report["config"]["beforeCommands"][0], "echo rc");
    assert_cmd(&report, "from-rc-main", "echo main");
}

#[test]
fn dotenv_loaded_without_dumping_env_into_dry_run() {
    let dir = fixture("dotenv");
    let output = run_in(&dir, &["--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("env-file-secret"), "{stdout}");
    let report = stdout_json(&output);
    assert_cmd(&report, "from-dotenv", "echo from-dotenv");
    assert_eq!(report["config"]["commands"][0]["cwd"], "./env-cwd");
}

#[test]
fn local_extends() {
    let report = dry_run_ok(&fixture("extends"), &["--dry-run"]);
    assert_eq!(report["config"]["beforeCommands"][0], "echo base");
    let cmds = report["config"]["commands"].as_array().unwrap();
    assert_eq!(cmds[0]["name"], "from-extends-child");
    assert_eq!(cmds[0]["command"], "echo child");
    assert_eq!(cmds[1]["name"], "from-extends-base");
    assert_eq!(cmds[1]["command"], "echo basecmd");
}

#[test]
fn node_env_overlay() {
    let output = run_in_with_env(
        &fixture("env_overlay"),
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
    assert_cmd(&report, "from-overlay", "echo overlay");
}

#[test]
fn loads_js_via_jiti() {
    assert_jiti_format("js", "stack.config.js", "from-js", "echo from-js");
}

#[test]
fn loads_ts_via_jiti() {
    assert_jiti_format("ts", "stack.config.ts", "from-ts", "echo from-ts");
}

#[test]
fn loads_mjs_via_jiti() {
    assert_jiti_format("mjs", "stack.config.mjs", "from-mjs", "echo from-mjs");
}

#[test]
fn loads_cjs_via_jiti() {
    assert_jiti_format("cjs", "stack.config.cjs", "from-cjs", "echo from-cjs");
}

#[test]
fn loads_mts_via_jiti() {
    assert_jiti_format("mts", "stack.config.mts", "from-mts", "echo from-mts");
}

#[test]
fn loads_cts_via_jiti() {
    assert_jiti_format("cts", "stack.config.cts", "from-cts", "echo from-cts");
}

#[test]
fn jiti_config_sees_interpolated_dotenv() {
    if skip_jiti() {
        return;
    }
    std::env::remove_var("APP_DIR");
    std::env::remove_var("BASE");
    let report = dry_run_lib(&fixture("dotenv_js"));
    std::env::remove_var("APP_DIR");
    std::env::remove_var("BASE");
    assert_cmd(&report, "from-dotenv-js", "/tmp/stackrun-dry/app");
    assert_eq!(
        report["config"]["commands"][0]["cwd"],
        "/tmp/stackrun-dry/app"
    );
}
