//! Optional live tunnel check. Skips unless `STACKRUN_LIVE_TUNNEL=1`.
use stackrun::config::types::{CfTunnelConfig, CommandEntry, Command, StackrunConfig};
use stackrun::stack;
use stackrun::tunnel::TunnelRuntime;

#[test]
fn live_tunnel_opt_in() {
    if std::env::var("STACKRUN_LIVE_TUNNEL").ok().as_deref() != Some("1") {
        return;
    }
    let config = StackrunConfig {
        tunnel_enabled: Some(true),
        cf_tunnel_config: Some(CfTunnelConfig {
            remove_existing_tunnel: Some(true),
            remove_existing_dns: Some(true),
            ..CfTunnelConfig::default()
        }),
        commands: Some(vec![CommandEntry::Full(Command {
            command: "sleep 1".into(),
            name: Some("sleep".into()),
            url: Some("http://127.0.0.1:9".into()),
            tunnel_url: std::env::var("STACKRUN_LIVE_HOSTNAME")
                .ok()
                .map(|h| format!("https://{h}")),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let _ = stack::run_with_tunnel(&config, TunnelRuntime::real());
}
