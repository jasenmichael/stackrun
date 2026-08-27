use stackrun::config::types::{
    CfTunnelConfig, CommandEntry, CommandSpec, ConcurrentlyOptions, StackrunConfig,
};
use stackrun::process;
use stackrun::Error;
use std::fs;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn missing_token_does_not_run_commands() {
    let _g = env_lock().lock().unwrap();
    std::env::remove_var("CF_TOKEN");
    std::env::remove_var("CLOUDFLARE_TOKEN");
    let dir = tempdir().unwrap();
    let marker = dir.path().join("ran");
    let config = StackrunConfig {
        tunnel_enabled: Some(true),
        commands: Some(vec![CommandEntry::Full(CommandSpec {
            command: format!("echo ran > {}", marker.display()),
            url: Some("http://localhost:9".into()),
            tunnel_url: Some("https://api.example.dev".into()),
            ..CommandSpec::default()
        })]),
        ..StackrunConfig::default()
    };
    let err = process::run(&config).unwrap_err();
    assert!(matches!(err, Error::CloudflareTokenRequired));
    assert!(!marker.exists());
}

#[test]
fn empty_ingress_aborts_before_commands() {
    let _g = env_lock().lock().unwrap();
    let dir = tempdir().unwrap();
    let marker = dir.path().join("before");
    let config = StackrunConfig {
        tunnel_enabled: Some(true),
        cf_tunnel_config: Some(CfTunnelConfig {
            cf_token: Some("tok".into()),
            ..CfTunnelConfig::default()
        }),
        before_commands: Some(vec![format!("echo x > {}", marker.display())]),
        commands: Some(vec![CommandEntry::Full(CommandSpec {
            command: "echo hi".into(),
            ..CommandSpec::default()
        })]),
        ..StackrunConfig::default()
    };
    let err = process::run(&config).unwrap_err();
    assert!(matches!(err, Error::NoTunnelIngress));
    assert!(!marker.exists());
}

#[test]
fn before_command_failure_skips_main() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("main");
    let config = StackrunConfig {
        before_commands: Some(vec!["sh -c 'exit 2'".into()]),
        commands: Some(vec![CommandEntry::Full(CommandSpec {
            command: format!("echo ran > {}", marker.display()),
            ..CommandSpec::default()
        })]),
        ..StackrunConfig::default()
    };
    let err = process::run(&config).unwrap_err();
    assert!(matches!(err, Error::BeforeCommandFailed { .. }));
    assert!(!marker.exists());
}

#[test]
fn after_commands_skipped_on_failure() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("after");
    let config = StackrunConfig {
        concurrently_options: Some(ConcurrentlyOptions {
            kill_others: Some(stackrun::config::types::KillOthers::One("failure".into())),
            ..ConcurrentlyOptions::default()
        }),
        after_commands: Some(vec![format!("echo after > {}", marker.display())]),
        commands: Some(vec![CommandEntry::Full(CommandSpec {
            command: "sh -c 'exit 7'".into(),
            name: Some("fail".into()),
            ..CommandSpec::default()
        })]),
        ..StackrunConfig::default()
    };
    let code = process::run(&config).expect("run");
    assert_eq!(code, 7);
    assert!(!marker.exists());
}

#[test]
fn tunnel_env_applied_when_enabled_via_effective_env() {
    use stackrun::config::types::EnvValue;
    use std::collections::BTreeMap;
    let mut env = BTreeMap::new();
    env.insert("MYVAR".into(), EnvValue::String("base".into()));
    let mut tunnel_env = BTreeMap::new();
    tunnel_env.insert("MYVAR".into(), EnvValue::String("tun".into()));
    let spec = CommandSpec {
        command: "true".into(),
        env: Some(env),
        tunnel_env: Some(tunnel_env),
        ..CommandSpec::default()
    };
    assert_eq!(spec.effective_env(false).get("MYVAR").unwrap(), "base");
    assert_eq!(spec.effective_env(true).get("MYVAR").unwrap(), "tun");
}

#[test]
fn command_env_reaches_child() {
    let dir = tempdir().unwrap();
    let out = dir.path().join("env.txt");
    let mut env = std::collections::BTreeMap::new();
    env.insert(
        "STACKRUN_PROBE".into(),
        stackrun::config::types::EnvValue::String("from-config".into()),
    );
    let config = StackrunConfig {
        commands: Some(vec![CommandEntry::Full(CommandSpec {
            command: format!("sh -c 'printf %s \"$STACKRUN_PROBE\" > {}'", out.display()),
            env: Some(env),
            ..CommandSpec::default()
        })]),
        ..StackrunConfig::default()
    };
    let code = process::run(&config).expect("run");
    assert_eq!(code, 0);
    assert_eq!(fs::read_to_string(&out).unwrap(), "from-config");
}

#[test]
fn handle_input_false_is_kept() {
    use stackrun::config::load::apply_defaults;
    let mut config = StackrunConfig {
        concurrently_options: Some(ConcurrentlyOptions {
            handle_input: Some(false),
            ..ConcurrentlyOptions::default()
        }),
        ..StackrunConfig::default()
    };
    apply_defaults(&mut config);
    assert_eq!(
        config.concurrently_options.as_ref().unwrap().handle_input,
        Some(false)
    );
}
