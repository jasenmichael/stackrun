use stackrun::config::types::CommandEntry;
use stackrun::config::{load_config, LoadOptions};
use std::fs;
use tempfile::tempdir;

#[test]
fn loads_json_jsonc_json5_yaml_toml() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - run: echo yaml\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert_eq!(loaded.config.runnable_commands()[0].run, "echo yaml");

    for (name, body, expect) in [
        (
            "a.json",
            r#"{"commands":[{"run":"echo json"}]}"#,
            "echo json",
        ),
        (
            "a.jsonc",
            "{ /* c */ \"commands\": [{ \"run\": \"echo jsonc\", }] }",
            "echo jsonc",
        ),
        (
            "a.json5",
            "{ commands: [{ run: 'echo json5' }] }",
            "echo json5",
        ),
        ("a.toml", "[[commands]]\nrun = \"echo toml\"\n", "echo toml"),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, body).unwrap();
        let mut opts = LoadOptions::for_cwd(dir.path());
        opts.config_file = path.to_string_lossy().into_owned();
        let loaded = load_config(opts).unwrap();
        assert_eq!(loaded.config.runnable_commands()[0].run, expect, "{name}");
    }
}

#[test]
fn discovers_stack_config_yaml_without_node() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        r#"
commands:
  - name: py
    run: python server.py
"#,
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    let cmds = loaded.config.runnable_commands();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name.as_deref(), Some("py"));
    assert_eq!(cmds[0].run, "python server.py");
}

#[test]
fn explicit_config_path() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stackrun.yaml"),
        "commands:\n  - run: echo from-explicit\n",
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
        loaded.config.runnable_commands()[0].run,
        "echo from-explicit"
    );
}

#[test]
fn rc_file_merges_under_main() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "tunnel: true\ncommands:\n  - run: echo main\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".stackrc"),
        "tunnel=false\nbefore[]=echo rc\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert!(
        !loaded.config.tunnel_enabled(),
        "top-level tunnel is ignored"
    );
    assert_eq!(loaded.config.before_commands(), &["echo rc".to_string()]);
}

#[test]
fn json_cli_overlay_wins() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "tunnel: false\ncommands:\n  - run: echo file\n",
    )
    .unwrap();
    let mut opts = LoadOptions::for_cwd(dir.path());
    opts.json_overlay = Some(r#"{"commands":[{"run":"echo overlay"}]}"#.into());
    let loaded = load_config(opts).unwrap();
    assert!(
        !loaded.config.tunnel_enabled(),
        "top-level tunnel overlay is ignored"
    );
    let runs: Vec<String> = loaded
        .config
        .runnable_commands()
        .iter()
        .map(|c| c.run.clone())
        .collect();
    assert!(
        runs.contains(&"echo overlay".into()),
        "json overlay commands merge: {runs:?}"
    );
    assert!(runs.contains(&"echo file".into()));
}

#[test]
fn command_flag_replaces_commands() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - run: echo file\n",
    )
    .unwrap();
    let mut opts = LoadOptions::for_cwd(dir.path());
    opts.command = Some("python server.py".into());
    let loaded = load_config(opts).unwrap();
    let cmds = loaded.config.runnable_commands();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].run, "python server.py");
}

#[test]
fn dotenv_does_not_override_existing_env() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".env"), "STACKRUN_TEST_DOTENV=fromfile\n").unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "commands:\n  - run: echo x\n",
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
before:
  - echo base
$development:
  before:
    - echo overlay-dev
commands:
  - run: echo x
"#,
    )
    .unwrap();
    std::env::set_var("NODE_ENV", "development");
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    std::env::remove_var("NODE_ENV");
    assert!(
        loaded
            .config
            .before_commands()
            .iter()
            .any(|s| s == "echo overlay-dev"),
        "NODE_ENV overlay should apply: {:?}",
        loaded.config.before_commands()
    );
}

#[test]
fn local_extends() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("base.yaml"),
        "before: [echo base]\ncommands:\n  - run: echo basecmd\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "extends: ./base.yaml\ncommands:\n  - run: echo child\n",
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
        .filter_map(CommandEntry::to_command)
        .map(|c| c.run)
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
        "commands:\n  - name: nope\n  - run: echo yes\n",
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
        "commands:\n  - run: echo hidden\n",
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert_eq!(loaded.config.runnable_commands()[0].run, "echo hidden");
}

#[test]
fn spec_tunnel_shape_maps_namable_tunnel() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        r#"
process:
  killOthers: failure
commands:
  - name: nuxt
    run: echo nuxt
    color: green
    tunnel:
      local: http://localhost:3000
      public: https://bugpin.example.dev
      resource: bugpin
      removeExisting: true
"#,
    )
    .unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert!(loaded.config.tunnel_enabled());
    let cmd = &loaded.config.runnable_commands()[0];
    assert_eq!(cmd.run, "echo nuxt");
    assert_eq!(cmd.color.as_deref(), Some("green"));
    assert_eq!(cmd.tunnel_local(), Some("http://localhost:3000"));
    assert_eq!(cmd.tunnel_public(), Some("https://bugpin.example.dev"));
    assert_eq!(cmd.sibling_log_prefix(10), "tunnel-nuxt");
    assert_eq!(cmd.named_tunnel_name().as_deref(), Some("bugpin"));
}

#[test]
fn rc_beats_extends_when_main_omits_key() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("base.yaml"),
        "tunnel: false\nbefore: [echo base]\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("stack.config.yaml"),
        "extends: ./base.yaml\ncommands:\n  - run: echo main\n",
    )
    .unwrap();
    fs::write(dir.path().join(".stackrc"), "tunnel=true\n").unwrap();
    let loaded = load_config(LoadOptions::for_cwd(dir.path())).unwrap();
    assert!(
        !loaded.config.tunnel_enabled(),
        "stack tunnel keys are ignored; no command has tunnel.local"
    );
    assert_eq!(loaded.config.before_commands(), &["echo base".to_string()]);
}
