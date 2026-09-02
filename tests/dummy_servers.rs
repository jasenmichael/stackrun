//! Two dummy HTTP servers through `load_config` + `stack::run`.
//!
//! Matches README “Two services”: stackrun starts more than one command at once.
//! `killOthers: failure` (load default) kills siblings; SIGINT stops every
//! command, then `afterCommands` run. A failed command still skips `afterCommands`.

use stackrun::config::{load_config, LoadOptions};
use stackrun::stack;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn python3() -> bool {
    Command::new("python3")
        .args(["-c", "import http.server"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn skip_python() -> bool {
    if python3() {
        false
    } else {
        eprintln!("skip: python3 + http.server not available");
        true
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display())
}

fn http_ok(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    if stream
        .write_all(b"GET / HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 96];
    let Ok(n) = stream.read(&mut buf) else {
        return false;
    };
    String::from_utf8_lossy(&buf[..n]).contains("200")
}

fn wait_http(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if http_ok(port) {
            return true;
        }
        thread::sleep(Duration::from_millis(30));
    }
    false
}

fn wait_down(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            return true;
        }
        thread::sleep(Duration::from_millis(30));
    }
    false
}

fn write_two_server_yaml(dir: &Path, api: u16, web: u16, fail: Option<&Path>, after: &Path) {
    let fail_line = match fail {
        Some(path) => format!(
            "  - name: fail\n    command: \"i=0; while [ ! -f {p} ]; do i=$((i+1)); if [ \\\"$i\\\" -ge 400 ]; then exit 1; fi; sleep 0.05; done; exit 1\"\n",
            p = quoted(path)
        ),
        None => String::new(),
    };
    let yaml = format!(
        r#"concurrentlyOptions:
  handleInput: false
beforeCommands:
  - printf 'ok' > index.html
afterCommands:
  - touch {after}
commands:
  - name: api
    command: python3 -m http.server {api} --bind 127.0.0.1
    prefixColor: green
  - name: web
    command: python3 -m http.server {web} --bind 127.0.0.1
    prefixColor: blue
{fail}"#,
        after = quoted(after),
        api = api,
        web = web,
        fail = fail_line,
    );
    fs::write(dir.join("stack.config.yaml"), yaml).expect("write config");
}

fn load_cwd(dir: &Path) -> stackrun::StackrunConfig {
    load_config(LoadOptions::for_cwd(dir))
        .expect("load_config")
        .config
}

#[test]
fn load_applies_kill_others_default_for_two_servers() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let after = dir.path().join("after");
    write_two_server_yaml(dir.path(), api, web, None, &after);
    let config = load_cwd(dir.path());
    assert!(!config.tunnel_enabled());
    assert!(
        config
            .process_options
            .as_ref()
            .is_some_and(|o| o.kill_others_on_failure()),
        "load default killOthers: failure"
    );
    assert_eq!(config.runnable_commands().len(), 2);
}

#[test]
fn two_http_servers_serve_then_kill_others_on_failure() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let fail = dir.path().join("go-fail");
    let after = dir.path().join("after");
    write_two_server_yaml(dir.path(), api, web, Some(&fail), &after);
    let config = load_cwd(dir.path());
    assert_eq!(config.runnable_commands().len(), 3);

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(stack::run(&config));
    });

    assert!(wait_http(api, Duration::from_secs(8)), "api never served");
    assert!(wait_http(web, Duration::from_secs(8)), "web never served");

    fs::write(&fail, b"").unwrap();

    let code = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("stack::run finished")
        .expect("stack::run");
    assert_ne!(code, 0, "fail sibling must make the run non-zero");
    assert!(wait_down(api, Duration::from_secs(5)), "api still up");
    assert!(wait_down(web, Duration::from_secs(5)), "web still up");
    assert!(
        !after.exists(),
        "afterCommands must skip when a command fails (killOthers: failure)"
    );
}

#[test]
fn two_http_servers_serve_then_sigint_stops_groups() {
    if skip_python() {
        return;
    }
    if !cfg!(unix) {
        eprintln!("skip: SIGINT process-group stop is Unix");
        return;
    }

    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let after = dir.path().join("after");
    write_two_server_yaml(dir.path(), api, web, None, &after);

    let mut child = Command::new(env!("CARGO_BIN_EXE_stackrun"))
        .current_dir(dir.path())
        .env_remove("TUNNEL")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn stackrun");

    let started = wait_http(api, Duration::from_secs(8)) && wait_http(web, Duration::from_secs(8));
    if !started {
        let _ = child.kill();
        let _ = child.wait();
        panic!("api/web never served before SIGINT");
    }

    let pid = child.id();
    let kill = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("kill -INT");
    assert!(kill.success(), "kill -INT {pid}");

    let status = wait_child(&mut child, Duration::from_secs(10));
    assert!(status, "stackrun did not exit after SIGINT");
    assert!(wait_down(api, Duration::from_secs(5)), "api still up");
    assert!(wait_down(web, Duration::from_secs(5)), "web still up");
    assert!(
        after.exists(),
        "afterCommands must run after Ctrl+C stops every command"
    );
}

fn wait_child(child: &mut std::process::Child, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(_) => return false,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    false
}
