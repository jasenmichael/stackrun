//! `stack::run_with_tunnel` with exported fake adapters (no live cloudflared).
//! Tunnel sibling is `true tunnel …` so spawn succeeds. User command is echo.

use stackrun::config::types::{
    Command, CommandEntry, CommandTunnel, ProcessOptions, StackrunConfig, TunnelDefaults,
    TunnelSetting,
};
use stackrun::stack;
use stackrun::tunnel::{MockCloudflared, TunnelRuntime};
use std::sync::Arc;

fn fake_bin() -> String {
    if cfg!(windows) {
        "echo".into()
    } else {
        "true".into()
    }
}

#[test]
fn run_with_fake_named_tunnel_spawns_user_command_and_cleans_up() {
    let cf = Arc::new(MockCloudflared {
        has_cert: true,
        binary: fake_bin(),
        ..MockCloudflared::default()
    });

    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Defaults(TunnelDefaults {
            remove_existing: Some(true),
            ..TunnelDefaults::default()
        })),
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo stack-ok".into(),
            name: Some("echo".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:9".into()),
                public: Some("https://api.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };

    let runtime = TunnelRuntime::from_arc(cf.clone());
    let code = stack::run_with_tunnel(&config, runtime).expect("run_with_tunnel");
    assert_eq!(code, 0);
    assert_eq!(cf.created.lock().unwrap().as_slice(), ["echo"]);
    assert_eq!(
        cf.routed.lock().unwrap().as_slice(),
        [("echo".into(), "api.example.dev".into(), true)]
    );
    assert_eq!(cf.deleted.lock().unwrap().as_slice(), ["echo"]);
}

#[test]
fn stack_resource_is_cloudflare_name_not_log_prefix() {
    let cf = Arc::new(MockCloudflared {
        has_cert: true,
        binary: fake_bin(),
        ..MockCloudflared::default()
    });

    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Defaults(TunnelDefaults {
            remove_existing: Some(true),
            prefix: Some("tunnel".into()),
            color: Some("cyan".into()),
            resource: Some("bugpin".into()),
        })),
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo stack-ok".into(),
            name: Some("nuxt".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:9".into()),
                public: Some("https://bugpin.example.dev".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };

    let runtime = TunnelRuntime::from_arc(cf.clone());
    let code = stack::run_with_tunnel(&config, runtime).expect("run_with_tunnel");
    assert_eq!(code, 0);
    assert_eq!(cf.created.lock().unwrap().as_slice(), ["bugpin"]);
    assert_eq!(
        cf.routed.lock().unwrap().as_slice(),
        [("bugpin".into(), "bugpin.example.dev".into(), true)]
    );
    assert_eq!(cf.deleted.lock().unwrap().as_slice(), ["bugpin"]);
}

#[test]
fn run_with_fake_quick_tunnel_skips_create_and_dns() {
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
    let code = stack::run_with_tunnel(&config, runtime).expect("run_with_tunnel");
    assert_eq!(code, 0);
    assert!(cf.created.lock().unwrap().is_empty());
    assert!(cf.routed.lock().unwrap().is_empty());
    assert!(cf.deleted.lock().unwrap().is_empty());
}
