use stackrun::config::load::{load_config, LoadOptions};
use stackrun::config::parse::parse_str;
use stackrun::config::types::CommandEntry;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn loads_json_jsonc_json5_yaml_toml() {
    let yaml = parse_str(
        Path::new("t.yaml"),
        "commands:\n  - name: api\n    command: python server.py\n    cwd: ./api\n",
    )
    .unwrap();
    assert_eq!(yaml["commands"][0]["command"], "python server.py");

    let toml = parse_str(
        Path::new("t.toml"),
        r#"
tunnelEnabled = false
[[commands]]
name = "web"
command = "npm run dev"
"#,
    )
    .unwrap();
    assert_eq!(toml["commands"][0]["name"], "web");

    let json = parse_str(
        Path::new("t.json"),
        r#"{"commands":[{"command":"echo json"}]}"#,
    )
    .unwrap();
    assert_eq!(json["commands"][0]["command"], "echo json");

    let jsonc = parse_str(
        Path::new("t.jsonc"),
        "{ /* c */ \"commands\": [{ \"command\": \"echo jsonc\", }] }",
    )
    .unwrap();
    assert_eq!(jsonc["commands"][0]["command"], "echo jsonc");

    let json5 = parse_str(
        Path::new("t.json5"),
        "{ commands: [{ command: 'echo json5' }] }",
    )
    .unwrap();
    assert_eq!(json5["commands"][0]["command"], "echo json5");
}

#[test]
fn discovers_stack_config_yaml_without_node() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        r#"
commands:
  - name: py
    command: python server.py
"#,
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    let cmds = loaded.config.runnable_commands();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name.as_deref(), Some("py"));
    assert_eq!(cmds[0].command, "python server.py");
}

#[test]
fn explicit_config_path() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stackrun.yaml"),
        "commands:\n  - command: echo from-explicit\n",
    )
    .unwrap();
    let mut opts = LoadOptions::for_cwd(dir.path());
    opts.config_file = dir
        .path()
        .join("stackrun.yaml")
        .to_string_lossy()
        .into_owned();
    let loaded = load_config(opts).unwrap();
    assert_eq!(
        loaded.config.runnable_commands()[0].command,
        "echo from-explicit"
    );
}

#[test]
fn rc_file_merges_under_main() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "tunnelEnabled: true\ncommands:\n  - command: echo main\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".stackrc"),
        "tunnelEnabled=false\nbeforeCommands[]=echo rc\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert!(loaded.config.tunnel_enabled());
    assert_eq!(loaded.config.before_commands(), &["echo rc".to_string()]);
}

#[test]
fn json_cli_overlay_wins() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "tunnelEnabled: false\ncommands:\n  - command: echo file\n",
    )
    .unwrap();
    let mut opts = LoadOptions::for_cwd(dir.path());
    opts.json_overlay = Some(r#"{"tunnelEnabled": true}"#.into());
    let loaded = load_config(opts).unwrap();
    assert!(loaded.config.tunnel_enabled());
    assert_eq!(loaded.config.runnable_commands()[0].command, "echo file");
}

#[test]
fn command_flag_replaces_commands() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - command: echo file\n",
    )
    .unwrap();
    let mut opts = LoadOptions::for_cwd(dir.path());
    opts.command = Some("python server.py".into());
    let loaded = load_config(opts).unwrap();
    let cmds = loaded.config.runnable_commands();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].command, "python server.py");
}

#[test]
fn dotenv_does_not_override_existing_env() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".env"), "STACKRUN_TEST_DOTENV=fromfile\n").unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - command: echo x\n",
    )
    .unwrap();
    std::env::set_var("STACKRUN_TEST_DOTENV", "already");
    load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert_eq!(std::env::var("STACKRUN_TEST_DOTENV").unwrap(), "already");
    std::env::remove_var("STACKRUN_TEST_DOTENV");
}

#[test]
fn env_specific_overlay() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        r#"
tunnelEnabled: false
$development:
  tunnelEnabled: true
commands:
  - command: echo x
"#,
    )
    .unwrap();
    std::env::set_var("NODE_ENV", "development");
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    std::env::remove_var("NODE_ENV");
    assert!(loaded.config.tunnel_enabled());
}

#[test]
fn local_extends() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("base.yaml"),
        "beforeCommands: [echo base]\ncommands:\n  - command: echo basecmd\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "extends: ./base.yaml\ncommands:\n  - command: echo child\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert_eq!(loaded.config.before_commands(), &["echo base".to_string()]);
    let cmds: Vec<_> = loaded
        .config
        .commands
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(CommandEntry::to_spec)
        .map(|c| c.command)
        .collect();
    // child commands first (preferred), then unique from base
    assert_eq!(
        cmds,
        vec!["echo child".to_string(), "echo basecmd".to_string()]
    );
}

#[test]
fn filters_entries_without_command() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - name: nope\n  - command: echo yes\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert_eq!(loaded.config.runnable_commands().len(), 1);
}

#[test]
fn config_dir_stack_yaml() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join(".config")).unwrap();
    fs::write(
        dir.path().join(".config/stack.yaml"),
        "commands:\n  - command: echo hidden\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert_eq!(loaded.config.runnable_commands()[0].command, "echo hidden");
}
