use stackrun::config::types::{Command, CommandEntry, StackrunConfig};
use stackrun::stack;

#[test]
fn runs_echo_command() {
    let config = StackrunConfig {
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo stackrun-ok".into(),
            name: Some("echo".into()),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let code = stack::run(&config).expect("stack run");
    assert_eq!(code, 0);
}
