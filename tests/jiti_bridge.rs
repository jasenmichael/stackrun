use stackrun::cli::JitiMode;
use stackrun::config::{load_config, LoadOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn formats_root() -> PathBuf {
    repo_root().join("tests/config_formats")
}

fn jiti_importable_from(cwd: &Path) -> bool {
    let Ok(status) = Command::new("node")
        .args(["--input-type=module", "-e", "await import('jiti')"])
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    else {
        return false;
    };
    status.success()
}

fn jiti_available() -> bool {
    jiti_importable_from(&repo_root())
}

fn which_node() -> bool {
    which_bin("node").is_some()
}

fn which_bin(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[test]
fn loads_js_and_ts_via_jiti_when_available() {
    if !jiti_available() {
        eprintln!("skip: node + jiti not available");
        return;
    }

    let loaded = load_config(LoadOptions::for_cwd(formats_root().join("ts"))).expect("ts config");
    let cmds = loaded.config.runnable_commands();
    assert_eq!(cmds[0].name.as_deref(), Some("from-ts"));
    assert_eq!(cmds[0].run, "echo from-ts");

    let loaded = load_config(LoadOptions::for_cwd(formats_root().join("js"))).expect("js config");
    assert_eq!(loaded.config.runnable_commands()[0].run, "echo from-js");
}

#[test]
fn missing_node_is_actionable() {
    if which_node() {
        eprintln!("skip: node is on PATH");
        return;
    }
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("stack.config.ts"), "export default {}").unwrap();
    let err = load_config(LoadOptions::for_cwd(dir.path())).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Node.js"), "{msg}");
}

#[test]
fn missing_local_jiti_is_actionable() {
    if !which_node() {
        eprintln!("skip: node is not on PATH");
        return;
    }
    let dir = tempdir().unwrap();
    if jiti_importable_from(dir.path()) {
        eprintln!("skip: jiti is resolvable from the temp dir");
        return;
    }
    fs::write(
        dir.path().join("stack.config.ts"),
        "export default { commands: [] }",
    )
    .unwrap();
    let err = load_config(LoadOptions::for_cwd(dir.path())).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("YAML"), "{msg}");
    assert!(msg.contains("npm i -D jiti"), "{msg}");
    assert!(msg.contains("--jiti npx"), "{msg}");
    assert!(msg.contains("STACKRUN_JITI=npx"), "{msg}");
    assert!(!msg.contains("global"), "{msg}");
    assert!(!msg.contains("npm i -g"), "{msg}");
}

#[cfg(unix)]
#[test]
fn missing_npx_is_actionable() {
    if !which_node() {
        eprintln!("skip: node is not on PATH");
        return;
    }
    let Some(node) = which_bin("node") else {
        return;
    };
    let dir = tempdir().unwrap();
    if jiti_importable_from(dir.path()) {
        eprintln!("skip: jiti is resolvable from the temp dir");
        return;
    }

    let bin_dir = dir.path().join("bin");
    fs::create_dir(&bin_dir).unwrap();
    std::os::unix::fs::symlink(&node, bin_dir.join("node")).unwrap();
    fs::write(
        dir.path().join("stack.config.ts"),
        "export default { commands: [] }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_stackrun"))
        .current_dir(dir.path())
        .env("PATH", &bin_dir)
        .env_remove("STACKRUN_JITI")
        .args(["--jiti", "npx", "--dry-run"])
        .output()
        .expect("spawn stackrun");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("npx"), "{stderr}");
    assert!(
        stderr.contains("npm") || stderr.contains("Node.js"),
        "{stderr}"
    );
    assert!(stderr.contains("YAML"), "{stderr}");
    assert!(!stderr.contains("global"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn jiti_npx_escape_hatch_without_network() {
    if !jiti_available() {
        eprintln!("skip: repo jiti not available to back the fake npx");
        return;
    }
    let Some(node) = which_bin("node") else {
        eprintln!("skip: node is not on PATH");
        return;
    };
    let dir = tempdir().unwrap();
    if jiti_importable_from(dir.path()) {
        eprintln!("skip: local jiti would succeed before npx");
        return;
    }

    let bin_dir = dir.path().join("bin");
    fs::create_dir(&bin_dir).unwrap();
    std::os::unix::fs::symlink(&node, bin_dir.join("node")).unwrap();

    let marker = dir.path().join("npx-invoked");
    let repo = repo_root();
    let npx = bin_dir.join("npx");
    fs::write(
        &npx,
        format!(
            r#"#!/bin/sh
if [ "$1" != "-p" ] || [ "${{2%%@*}}" != "jiti" ]; then
  echo "fake-npx: expected -p jiti, got $*" >&2
  exit 90
fi
shift 2
printf invoked > "{marker}"
cd "{repo}"
exec "$@"
"#,
            marker = marker.display(),
            repo = repo.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&npx, fs::Permissions::from_mode(0o755)).unwrap();

    fs::write(
        dir.path().join("stack.config.ts"),
        r#"export default { commands: [{ name: "npx", run: "echo via-npx" }] };"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_stackrun"))
        .current_dir(dir.path())
        .env("PATH", &bin_dir)
        .env_remove("STACKRUN_JITI")
        .args(["--jiti", "npx", "--dry-run"])
        .output()
        .expect("spawn stackrun");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(marker.is_file(), "fake npx was not invoked");
    let json: serde_json::Value =
        serde_json::from_str(std::str::from_utf8(&output.stdout).unwrap().trim()).unwrap();
    assert_eq!(json["config"]["commands"][0]["run"], "echo via-npx");
}

#[cfg(unix)]
#[test]
fn stackrun_jiti_env_selects_npx() {
    if !jiti_available() {
        eprintln!("skip: repo jiti not available to back the fake npx");
        return;
    }
    let Some(node) = which_bin("node") else {
        return;
    };
    let dir = tempdir().unwrap();
    if jiti_importable_from(dir.path()) {
        eprintln!("skip: local jiti would succeed before npx");
        return;
    }

    let bin_dir = dir.path().join("bin");
    fs::create_dir(&bin_dir).unwrap();
    std::os::unix::fs::symlink(&node, bin_dir.join("node")).unwrap();

    let marker = dir.path().join("npx-invoked");
    let repo = repo_root();
    let npx = bin_dir.join("npx");
    fs::write(
        &npx,
        format!(
            r#"#!/bin/sh
if [ "$1" != "-p" ] || [ "${{2%%@*}}" != "jiti" ]; then
  echo "fake-npx: expected -p jiti, got $*" >&2
  exit 90
fi
shift 2
printf invoked > "{marker}"
cd "{repo}"
exec "$@"
"#,
            marker = marker.display(),
            repo = repo.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&npx, fs::Permissions::from_mode(0o755)).unwrap();

    fs::write(
        dir.path().join("stack.config.js"),
        r#"export default { commands: [{ name: "env", run: "echo via-env" }] };"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_stackrun"))
        .current_dir(dir.path())
        .env("PATH", &bin_dir)
        .env("STACKRUN_JITI", "npx")
        .args(["--dry-run"])
        .output()
        .expect("spawn stackrun");
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(marker.is_file(), "fake npx was not invoked");
}

#[test]
fn library_npx_mode_without_npx_errors() {
    if !which_node() {
        eprintln!("skip: node is not on PATH");
        return;
    }
    let dir = tempdir().unwrap();
    if jiti_importable_from(dir.path()) {
        eprintln!("skip: jiti is resolvable from the temp dir");
        return;
    }
    if which_bin("npx").is_some() {
        eprintln!("skip: npx is on PATH (cannot assert NpxRequired)");
        return;
    }
    fs::write(
        dir.path().join("stack.config.ts"),
        "export default { commands: [] }",
    )
    .unwrap();
    let mut opts = LoadOptions::for_cwd(dir.path());
    opts.jiti = JitiMode::Npx;
    let err = load_config(opts).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("npx"), "{msg}");
    assert!(msg.contains("YAML"), "{msg}");
}
