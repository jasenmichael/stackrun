use stackrun::config::types::{CommandEntry, CommandSpec, StackrunConfig};
use stackrun::process;

#[test]
fn runs_echo_command() {
    let config = StackrunConfig {
        commands: Some(vec![CommandEntry::Full(CommandSpec {
            command: "echo stackrun-ok".into(),
            name: Some("echo".into()),
            ..CommandSpec::default()
        })]),
        ..StackrunConfig::default()
    };
    let code = process::run(&config).expect("process run");
    assert_eq!(code, 0);
}
