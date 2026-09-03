//! Optional live tunnel check. Skips unless `STACKRUN_LIVE_TUNNEL=1`.
use stackrun::config::types::{
    Command, CommandEntry, CommandTunnel, StackrunConfig, TunnelDefaults, TunnelSetting,
};
use stackrun::stack;
use stackrun::tunnel::TunnelRuntime;

#[test]
fn live_tunnel_opt_in() {
    if std::env::var("STACKRUN_LIVE_TUNNEL").ok().as_deref() != Some("1") {
        return;
    }
    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Defaults(TunnelDefaults {
            remove_existing: Some(true),
            ..TunnelDefaults::default()
        })),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "sleep 1".into(),
            name: Some("sleep".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:9".into()),
                public: std::env::var("STACKRUN_LIVE_HOSTNAME")
                    .ok()
                    .map(|h| format!("https://{h}")),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let _ = stack::run_with_tunnel(&config, TunnelRuntime::real());
}
