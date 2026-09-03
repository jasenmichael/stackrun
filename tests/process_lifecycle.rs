use stackrun::config::types::{
    Command, CommandEntry, CommandTunnel, ProcessOptions, StackrunConfig, TunnelSetting,
};
use stackrun::stack;
use stackrun::tunnel::{MockCloudflared, TunnelRuntime};
use stackrun::Error;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

fn fake_bin() -> String {
    if cfg!(windows) {
        "echo".into()
    } else {
        "true".into()
    }
}

fn mock_runtime(cf: MockCloudflared) -> TunnelRuntime {
    TunnelRuntime::from_parts(cf)
}

#[test]
fn missing_cloudflared_does_not_run_commands() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("ran");
    let before = dir.path().join("before");
    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Flag(true)),
        before: Some(vec![format!("echo x > {}", before.display())]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: format!("echo ran > {}", marker.display()),
            name: Some("api".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://localhost:9".into()),
                public: Some("https://api.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let cf = MockCloudflared {
        missing_binary: true,
        ..MockCloudflared::default()
    };
    let err = stack::run_with_tunnel(&config, mock_runtime(cf)).unwrap_err();
    assert!(matches!(err, Error::CloudflaredMissing));
    assert!(!marker.exists());
    assert!(!before.exists());
}

#[test]
fn empty_ingress_aborts_before_commands() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("before");
    let config = StackrunConfig {
        force_tunnel: true,
        tunnel: Some(TunnelSetting::Flag(true)),
        before: Some(vec![format!("echo x > {}", marker.display())]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo hi".into(),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let cf = MockCloudflared {
        binary: fake_bin(),
        ..MockCloudflared::default()
    };
    let err = stack::run_with_tunnel(&config, mock_runtime(cf)).unwrap_err();
    assert!(matches!(err, Error::NoTunnelIngress));
    assert!(!marker.exists());
}

#[test]
fn before_command_failure_skips_main() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("main");
    let config = StackrunConfig {
        before: Some(vec!["sh -c 'exit 2'".into()]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: format!("echo ran > {}", marker.display()),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let err = stack::run(&config).unwrap_err();
    assert!(matches!(err, Error::BeforeCommandFailed { .. }));
    assert!(!marker.exists());
}

#[test]
fn after_commands_skipped_on_failure() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("after");
    let config = StackrunConfig {
        process: Some(ProcessOptions {
            kill_others: Some(stackrun::config::types::KillOthers::One("failure".into())),
            ..ProcessOptions::default()
        }),
        after: Some(vec![format!("echo after > {}", marker.display())]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "sh -c 'exit 7'".into(),
            name: Some("fail".into()),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let code = stack::run(&config).expect("run");
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
    let spec = Command {
        run: "true".into(),
        env: Some(env),
        tunnel: Some(CommandTunnel {
            env: Some(tunnel_env),
            ..CommandTunnel::default()
        }),
        ..Command::default()
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
        commands: Some(vec![CommandEntry::Full(Command {
            run: format!("sh -c 'printf %s \"$STACKRUN_PROBE\" > {}'", out.display()),
            env: Some(env),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let code = stack::run(&config).expect("run");
    assert_eq!(code, 0);
    assert_eq!(fs::read_to_string(&out).unwrap(), "from-config");
}

#[test]
fn handle_input_false_is_kept() {
    use stackrun::apply_defaults;
    let mut config = StackrunConfig {
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        ..StackrunConfig::default()
    };
    apply_defaults(&mut config);
    assert_eq!(config.process.as_ref().unwrap().handle_input, Some(false));
}

#[test]
fn named_missing_cert_aborts_before_hooks() {
    let dir = tempdir().unwrap();
    let before = dir.path().join("before");
    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Flag(true)),
        before: Some(vec![format!("echo x > {}", before.display())]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo hi".into(),
            name: Some("api".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:9".into()),
                public: Some("https://api.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let cf = MockCloudflared {
        has_cert: false,
        binary: fake_bin(),
        ..MockCloudflared::default()
    };
    let err = stack::run_with_tunnel(&config, mock_runtime(cf)).unwrap_err();
    assert!(matches!(err, Error::CloudflaredLoginRequired { .. }));
    assert!(!before.exists());
}

#[test]
fn quick_only_runs_without_cert() {
    let cf = Arc::new(MockCloudflared {
        has_cert: false,
        binary: fake_bin(),
        ..MockCloudflared::default()
    });
    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Defaults(Default::default())),
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo quick-ok".into(),
            name: Some("web".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:3000".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let runtime = TunnelRuntime::from_arc(cf.clone());
    let code = stack::run_with_tunnel(&config, runtime).expect("run");
    assert_eq!(code, 0);
    assert!(cf.created.lock().unwrap().is_empty());
    assert!(cf.routed.lock().unwrap().is_empty());
}
