//! `stack::run_with_tunnel` with exported fake adapters (no live cloudflared).
//! Tunnel sibling is `true tunnel run …` so spawn succeeds. User command is echo.

use stackrun::config::types::{CfTunnelConfig, Command, CommandEntry, ProcessOptions, StackrunConfig};
use stackrun::stack;
use stackrun::tunnel::{MockCloudflared, RecordingDns, TunnelRuntime};
use tempfile::tempdir;

#[test]
fn run_with_fake_tunnel_spawns_user_command_and_cleans_up() {
    let dir = tempdir().unwrap();
    let cfg_dir = dir.path().join("cf");
    std::fs::create_dir_all(&cfg_dir).unwrap();

    let config = StackrunConfig {
        tunnel_enabled: Some(true),
        process_options: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        cf_tunnel_config: Some(CfTunnelConfig {
            cf_token: Some("tok".into()),
            cloudflared_config_dir: Some(cfg_dir.display().to_string()),
            remove_existing_tunnel: Some(true),
            remove_existing_dns: Some(true),
            ..CfTunnelConfig::default()
        }),
        commands: Some(vec![CommandEntry::Full(Command {
            command: "echo stack-ok".into(),
            name: Some("echo".into()),
            url: Some("http://127.0.0.1:9".into()),
            tunnel_url: Some("https://api.example.dev".into()),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };

    let cf = MockCloudflared {
        has_cert: true,
        binary: if cfg!(windows) {
            "echo".into()
        } else {
            "true".into()
        },
        ..MockCloudflared::default()
    };
    let runtime = TunnelRuntime::from_parts(cf, RecordingDns::default());
    let code = stack::run_with_tunnel(&config, runtime).expect("run_with_tunnel");
    assert_eq!(code, 0);
    assert!(
        !cfg_dir.join("config.yml").exists(),
        "cleanup must delete local cloudflared config.yml"
    );
}
