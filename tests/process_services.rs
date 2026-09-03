//! Concurrent dummy services plus before/after hooks via real `stack::run`.
//!
//! Does not repeat token-abort, empty-ingress, env, or handleInput cases from
//! `process_lifecycle.rs`. Overlap and sibling-kill are asserted with markers,
//! not only exit codes.

use stackrun::config::types::{Command, CommandEntry, KillOthers, ProcessOptions, StackrunConfig};
use stackrun::stack;
use stackrun::Error;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn spec(name: &str, command: impl Into<String>) -> CommandEntry {
    CommandEntry::Full(Command {
        run: command.into(),
        name: Some(name.into()),
        ..Command::default()
    })
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display())
}

fn wait_exists(path: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

fn hold_until_go(start: &Path, go: &Path) -> String {
    format!(
        "touch {start}; i=0; while [ ! -f {go} ]; do i=$((i+1)); if [ \"$i\" -ge 200 ]; then exit 1; fi; sleep 0.05; done",
        start = quoted(start),
        go = quoted(go),
    )
}

fn run_with_timeout(
    config: StackrunConfig,
    timeout: Duration,
) -> Result<Result<u8, Error>, mpsc::RecvTimeoutError> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(stack::run(&config));
    });
    rx.recv_timeout(timeout)
}

#[test]
fn multiple_dummies_overlap_then_after_runs() {
    let dir = tempdir().unwrap();
    let start_a = dir.path().join("start_a");
    let start_b = dir.path().join("start_b");
    let go = dir.path().join("go");
    let after = dir.path().join("after");

    let config = StackrunConfig {
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        commands: Some(vec![
            spec("svc-a", hold_until_go(&start_a, &go)),
            spec("svc-b", hold_until_go(&start_b, &go)),
        ]),
        after: Some(vec![format!("touch {}", quoted(&after))]),
        ..StackrunConfig::default()
    };

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(stack::run(&config));
    });

    assert!(
        wait_exists(&start_a, Duration::from_secs(5)),
        "svc-a never wrote start marker"
    );
    assert!(
        wait_exists(&start_b, Duration::from_secs(5)),
        "svc-b never wrote start marker"
    );
    assert!(
        start_a.exists() && start_b.exists(),
        "both dummies must be alive at once"
    );
    assert!(
        !after.exists(),
        "afterCommands must not run while dummies still held"
    );

    fs::write(&go, b"").unwrap();

    let code = rx
        .recv_timeout(Duration::from_secs(8))
        .expect("stack::run finished")
        .expect("stack::run");
    assert_eq!(code, 0);
    assert!(
        after.exists(),
        "afterCommands must run when all dummies exit 0"
    );
}

#[test]
fn before_commands_finish_before_any_dummy_starts() {
    let dir = tempdir().unwrap();
    let before1 = dir.path().join("before1");
    let before2 = dir.path().join("before2");
    let before2_early = dir.path().join("before2_early");
    let before_saw_svc = dir.path().join("before_saw_svc");
    let svc_a = dir.path().join("svc_a");
    let svc_b = dir.path().join("svc_b");
    let svc_a_early = dir.path().join("svc_a_early");
    let svc_b_early = dir.path().join("svc_b_early");

    let config = StackrunConfig {
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        before: Some(vec![
            format!("touch {}", quoted(&before1)),
            format!(
                "if [ ! -f {before1} ]; then touch {early}; fi; if [ -f {svc_a} ] || [ -f {svc_b} ]; then touch {leak}; fi; touch {before2}",
                before1 = quoted(&before1),
                early = quoted(&before2_early),
                svc_a = quoted(&svc_a),
                svc_b = quoted(&svc_b),
                leak = quoted(&before_saw_svc),
                before2 = quoted(&before2),
            ),
        ]),
        commands: Some(vec![
            spec(
                "svc-a",
                format!(
                    "if [ ! -f {before2} ]; then touch {early}; fi; touch {svc}",
                    before2 = quoted(&before2),
                    early = quoted(&svc_a_early),
                    svc = quoted(&svc_a),
                ),
            ),
            spec(
                "svc-b",
                format!(
                    "if [ ! -f {before2} ]; then touch {early}; fi; touch {svc}",
                    before2 = quoted(&before2),
                    early = quoted(&svc_b_early),
                    svc = quoted(&svc_b),
                ),
            ),
        ]),
        ..StackrunConfig::default()
    };

    let code = stack::run(&config).expect("run");
    assert_eq!(code, 0);
    assert!(before1.exists());
    assert!(before2.exists());
    assert!(svc_a.exists());
    assert!(svc_b.exists());
    assert!(
        !before2_early.exists(),
        "beforeCommands must run sequentially"
    );
    assert!(
        !before_saw_svc.exists(),
        "no dummy marker may exist before beforeCommands finish"
    );
    assert!(
        !svc_a_early.exists() && !svc_b_early.exists(),
        "dummies must not start before the last beforeCommand"
    );
}

#[test]
fn failing_before_skips_dummy_services() {
    let dir = tempdir().unwrap();
    let svc_a = dir.path().join("svc_a");
    let svc_b = dir.path().join("svc_b");

    let config = StackrunConfig {
        process: Some(ProcessOptions {
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        before: Some(vec!["exit 2".into()]),
        commands: Some(vec![
            spec("svc-a", format!("touch {}", quoted(&svc_a))),
            spec("svc-b", format!("touch {}", quoted(&svc_b))),
        ]),
        ..StackrunConfig::default()
    };

    let err = stack::run(&config).unwrap_err();
    assert!(matches!(err, Error::BeforeCommandFailed { .. }));
    assert!(!svc_a.exists(), "svc-a must not spawn after before failure");
    assert!(!svc_b.exists(), "svc-b must not spawn after before failure");
}

#[test]
fn failing_dummy_runs_after_and_kills_siblings() {
    let dir = tempdir().unwrap();
    let start_a = dir.path().join("start_a");
    let start_b = dir.path().join("start_b");
    let survived = dir.path().join("survived");
    let after = dir.path().join("after");

    let config = StackrunConfig {
        process: Some(ProcessOptions {
            kill_others: Some(KillOthers::One("failure".into())),
            handle_input: Some(false),
            ..ProcessOptions::default()
        }),
        after: Some(vec![format!("touch {}", quoted(&after))]),
        commands: Some(vec![
            spec(
                "hold",
                format!(
                    "touch {start}; sleep 30; touch {survived}",
                    start = quoted(&start_a),
                    survived = quoted(&survived),
                ),
            ),
            spec(
                "fail",
                format!(
                    "i=0; while [ ! -f {start_a} ]; do i=$((i+1)); if [ \"$i\" -ge 200 ]; then exit 1; fi; sleep 0.05; done; touch {start_b}; exit 1",
                    start_a = quoted(&start_a),
                    start_b = quoted(&start_b),
                ),
            ),
        ]),
        ..StackrunConfig::default()
    };

    let result = run_with_timeout(config, Duration::from_secs(8)).expect("stack::run finished");
    let code = result.expect("stack::run");
    assert_ne!(code, 0);
    assert!(start_a.exists(), "holding dummy must have started");
    assert!(start_b.exists(), "failing dummy must have started");
    assert!(
        !survived.exists(),
        "killOthers: failure must kill the holding sibling before it finishes"
    );
    assert!(after.exists(), "after must run when a dummy exits non-zero");
}
