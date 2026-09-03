//! Two `python3 -m http.server` processes with per-command tunnel variants.
//! Fake cloudflared only (`MockCloudflared`). No live Cloudflare.

use stackrun::config::types::{
    Command, CommandEntry, CommandTunnel, EnvValue, ProcessOptions, StackrunConfig, TunnelDefaults,
    TunnelSetting,
};
use stackrun::stack;
use stackrun::tunnel::{MockCloudflared, TunnelRuntime};
use stackrun::Error;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn python3() -> bool {
    StdCommand::new("python3")
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

fn fake_bin() -> String {
    if cfg!(windows) {
        "echo".into()
    } else {
        "true".into()
    }
}

fn mock_cf(has_cert: bool, missing: bool) -> Arc<MockCloudflared> {
    Arc::new(MockCloudflared {
        has_cert,
        missing_binary: missing,
        binary: fake_bin(),
        ..MockCloudflared::default()
    })
}

fn http_cmd(name: &str, port: u16, color: &str, tunnel: Option<CommandTunnel>) -> Command {
    Command {
        run: format!("python3 -m http.server {port} --bind 127.0.0.1"),
        name: Some(name.into()),
        color: Some(color.into()),
        tunnel,
        ..Command::default()
    }
}

fn fail_cmd(go: &Path) -> Command {
    Command {
        run: format!(
            "i=0; while [ ! -f {p} ]; do i=$((i+1)); if [ \"$i\" -ge 400 ]; then exit 1; fi; sleep 0.05; done; exit 1",
            p = quoted(go)
        ),
        name: Some("fail".into()),
        ..Command::default()
    }
}

fn base_config(
    commands: Vec<Command>,
    tunnel: TunnelSetting,
    before: Option<Vec<String>>,
) -> StackrunConfig {
    StackrunConfig {
        tunnel: Some(tunnel),
        process: Some(ProcessOptions {
            handle_input: Some(false),
            kill_others: Some(stackrun::config::types::KillOthers::One("failure".into())),
            ..ProcessOptions::default()
        }),
        before,
        commands: Some(commands.into_iter().map(CommandEntry::Full).collect()),
        ..StackrunConfig::default()
    }
}

fn probe_then_fail(config: StackrunConfig, runtime: TunnelRuntime, api: u16, web: u16, go: &Path) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(stack::run_with_tunnel(&config, runtime));
    });
    assert!(wait_http(api, Duration::from_secs(8)), "api never served");
    assert!(wait_http(web, Duration::from_secs(8)), "web never served");
    fs::write(go, b"").unwrap();
    let _ = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("stack::run finished");
}

#[test]
fn configured_tunnel_false_serves_without_cloudflared_or_tunnel_env() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let env_api = dir.path().join("env-api.txt");
    let mut env = BTreeMap::new();
    env.insert(
        "STACKRUN_TUNNEL_MARK".into(),
        EnvValue::String("should-not-appear".into()),
    );
    let cf = mock_cf(true, true);
    let config = base_config(
        vec![
            Command {
                run: format!(
                    "sh -c 'printf %s \"${{STACKRUN_TUNNEL_MARK-UNSET}}\" > {out}; exec python3 -m http.server {port} --bind 127.0.0.1'",
                    out = quoted(&env_api),
                    port = api
                ),
                name: Some("api".into()),
                color: Some("green".into()),
                tunnel: Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{api}")),
                    public: Some("https://api.example.dev".into()),
                    env: Some(env),
                    ..CommandTunnel::default()
                }),
                ..Command::default()
            },
            http_cmd(
                "web",
                web,
                "blue",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{web}")),
                    ..CommandTunnel::default()
                }),
            ),
            fail_cmd(&go),
        ],
        TunnelSetting::Flag(false),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf.clone()), api, web, &go);
    assert!(
        cf.created.lock().unwrap().is_empty(),
        "tunnel: false must not create tunnels"
    );
    let mark = fs::read_to_string(&env_api).unwrap_or_default();
    assert_eq!(
        mark, "UNSET",
        "tunnel.env must not apply when tunnel is off"
    );
}

#[test]
fn both_named_fakes_create_and_route_twice() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let cf = mock_cf(true, false);
    let config = base_config(
        vec![
            http_cmd(
                "api",
                api,
                "green",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{api}")),
                    public: Some("https://api.example.dev".into()),
                    ..CommandTunnel::default()
                }),
            ),
            http_cmd(
                "web",
                web,
                "blue",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{web}")),
                    public: Some("https://web.example.dev".into()),
                    ..CommandTunnel::default()
                }),
            ),
            fail_cmd(&go),
        ],
        TunnelSetting::Defaults(TunnelDefaults {
            remove_existing: Some(true),
            ..TunnelDefaults::default()
        }),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf.clone()), api, web, &go);
    let created = cf.created.lock().unwrap().clone();
    assert_eq!(created, vec!["api".to_string(), "web".to_string()]);
    let routed = cf.routed.lock().unwrap().clone();
    assert_eq!(
        routed,
        vec![
            ("api".into(), "api.example.dev".into(), true),
            ("web".into(), "web.example.dev".into(), true),
        ]
    );
    let deleted = cf.deleted.lock().unwrap().clone();
    assert!(deleted.contains(&"api".to_string()));
    assert!(deleted.contains(&"web".to_string()));
}

#[test]
fn named_plus_quick_one_create_one_url() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let cf = mock_cf(true, false);
    let config = base_config(
        vec![
            http_cmd(
                "api",
                api,
                "green",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{api}")),
                    public: Some("https://api.example.dev".into()),
                    ..CommandTunnel::default()
                }),
            ),
            http_cmd(
                "web",
                web,
                "blue",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{web}")),
                    ..CommandTunnel::default()
                }),
            ),
            fail_cmd(&go),
        ],
        TunnelSetting::Defaults(TunnelDefaults {
            remove_existing: Some(true),
            ..TunnelDefaults::default()
        }),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf.clone()), api, web, &go);
    assert_eq!(cf.created.lock().unwrap().as_slice(), ["api"]);
    assert_eq!(
        cf.routed.lock().unwrap().as_slice(),
        [("api".into(), "api.example.dev".into(), true)]
    );
}

#[test]
fn both_quick_zero_create() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let cf = mock_cf(false, false);
    let config = base_config(
        vec![
            http_cmd(
                "api",
                api,
                "green",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{api}")),
                    ..CommandTunnel::default()
                }),
            ),
            http_cmd(
                "web",
                web,
                "blue",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{web}")),
                    ..CommandTunnel::default()
                }),
            ),
            fail_cmd(&go),
        ],
        TunnelSetting::Defaults(Default::default()),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf.clone()), api, web, &go);
    assert!(cf.created.lock().unwrap().is_empty());
    assert!(cf.routed.lock().unwrap().is_empty());
}

#[test]
fn only_api_has_tunnel() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let cf = mock_cf(true, false);
    let config = base_config(
        vec![
            http_cmd(
                "api",
                api,
                "green",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{api}")),
                    public: Some("https://api.example.dev".into()),
                    ..CommandTunnel::default()
                }),
            ),
            http_cmd("web", web, "blue", None),
            fail_cmd(&go),
        ],
        TunnelSetting::Defaults(TunnelDefaults {
            remove_existing: Some(true),
            ..TunnelDefaults::default()
        }),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf.clone()), api, web, &go);
    assert_eq!(cf.created.lock().unwrap().as_slice(), ["api"]);
    assert_eq!(cf.routed.lock().unwrap().len(), 1);
}

#[test]
fn tunnel_flag_without_local_aborts_before_hooks() {
    let dir = tempdir().unwrap();
    let before = dir.path().join("before");
    let config = StackrunConfig {
        force_tunnel: true,
        tunnel: Some(TunnelSetting::Flag(true)),
        before: Some(vec![format!("echo x > {}", before.display())]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo hi".into(),
            name: Some("api".into()),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let err =
        stack::run_with_tunnel(&config, TunnelRuntime::from_arc(mock_cf(true, false))).unwrap_err();
    assert!(matches!(err, Error::NoTunnelIngress));
    assert!(!before.exists());
}

#[test]
fn tunnel_on_missing_cloudflared_aborts_before_hooks() {
    let dir = tempdir().unwrap();
    let before = dir.path().join("before");
    let config = StackrunConfig {
        tunnel: Some(TunnelSetting::Defaults(Default::default())),
        before: Some(vec![format!("echo x > {}", before.display())]),
        commands: Some(vec![CommandEntry::Full(Command {
            run: "echo hi".into(),
            name: Some("api".into()),
            tunnel: Some(CommandTunnel {
                local: Some("http://127.0.0.1:9".into()),
                ..CommandTunnel::default()
            }),
            ..Command::default()
        })]),
        ..StackrunConfig::default()
    };
    let err =
        stack::run_with_tunnel(&config, TunnelRuntime::from_arc(mock_cf(false, true))).unwrap_err();
    assert!(matches!(err, Error::CloudflaredMissing));
    assert!(!before.exists());
}

#[test]
fn tunnel_off_without_cloudflared_still_serves() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let cf = mock_cf(false, true);
    let config = base_config(
        vec![
            http_cmd("api", api, "green", None),
            http_cmd("web", web, "blue", None),
            fail_cmd(&go),
        ],
        TunnelSetting::Flag(false),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf), api, web, &go);
}

#[test]
fn quick_only_without_cert_still_runs() {
    if skip_python() {
        return;
    }
    let dir = tempdir().unwrap();
    let api = free_port();
    let web = free_port();
    let go = dir.path().join("go-fail");
    let cf = mock_cf(false, false);
    let config = base_config(
        vec![
            http_cmd(
                "api",
                api,
                "green",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{api}")),
                    ..CommandTunnel::default()
                }),
            ),
            http_cmd(
                "web",
                web,
                "blue",
                Some(CommandTunnel {
                    local: Some(format!("http://127.0.0.1:{web}")),
                    ..CommandTunnel::default()
                }),
            ),
            fail_cmd(&go),
        ],
        TunnelSetting::Defaults(Default::default()),
        None,
    );
    probe_then_fail(config, TunnelRuntime::from_arc(cf.clone()), api, web, &go);
    assert!(cf.created.lock().unwrap().is_empty());
}
