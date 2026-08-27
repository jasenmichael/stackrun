use stackrun::config::load::{load_config, LoadOptions};
use std::fs;
use std::process::Command;
use tempfile::tempdir;

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
fn loads_js_and_ts_via_jiti_when_available() {
    if !jiti_available() {
        eprintln!("skip: node + jiti not available");
        return;
    }

    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.ts"),
        r#"export default { commands: [{ name: "ts", command: "echo from-ts" }] };"#,
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).expect("ts config");
    let cmds = loaded.config.runnable_commands();
    assert_eq!(cmds[0].name.as_deref(), Some("ts"));
    assert_eq!(cmds[0].command, "echo from-ts");

    let dir2 = tempdir().unwrap();
    fs::write(
        dir2.path().join("stack.config.js"),
        r#"export default { commands: [{ name: "js", command: "echo from-js" }] };"#,
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir2.path())).expect("js config");
    assert_eq!(loaded.config.runnable_commands()[0].command, "echo from-js");
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

fn which_node() -> bool {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}
