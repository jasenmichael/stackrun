use super::cloudflared::{default_config_dir, CloudflaredOps};
use super::TunnelRuntime;
use crate::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Named-tunnel resource for one command: id, creds dir. Not a log prefix.
#[derive(Debug)]
pub struct TunnelSession {
    pub config_dir: PathBuf,
    pub tunnel_name: String,
    pub tunnel_id: String,
    pub hostname: String,
    pub local: String,
    pub binary: String,
}

pub fn hostname_from_public(public: &str) -> String {
    public
        .trim()
        .strip_prefix("https://")
        .or_else(|| public.trim().strip_prefix("http://"))
        .unwrap_or(public.trim())
        .to_string()
}

pub fn setup_named(
    runtime: &TunnelRuntime,
    tunnel_name: &str,
    local: &str,
    public: &str,
    remove_existing: bool,
) -> Result<TunnelSession, Error> {
    let config_dir = default_config_dir();
    let _ = fs::create_dir_all(&config_dir);

    let binary = runtime.cloudflared.binary_path()?;
    if !runtime.cloudflared.has_cert(&config_dir) {
        return Err(Error::CloudflaredLoginRequired {
            dir: config_dir.display().to_string(),
        });
    }

    let hostname = hostname_from_public(public);
    delete_named_tunnel(
        runtime.cloudflared.as_ref(),
        &config_dir,
        tunnel_name,
        remove_existing,
        false,
    )?;

    let tunnel_id = runtime.cloudflared.create_tunnel(tunnel_name)?;
    runtime
        .cloudflared
        .route_dns(tunnel_name, &hostname, remove_existing)?;

    Ok(TunnelSession {
        config_dir,
        tunnel_name: tunnel_name.to_string(),
        tunnel_id,
        hostname,
        local: local.to_string(),
        binary,
    })
}

pub fn cleanup(runtime: &TunnelRuntime, session: &TunnelSession) {
    info!("Cleaning up named tunnel: {}", session.tunnel_name);
    if let Err(err) = delete_named_tunnel(
        runtime.cloudflared.as_ref(),
        &session.config_dir,
        &session.tunnel_name,
        true,
        true,
    ) {
        warn!("tunnel delete: {err}");
    }
    let cred = session
        .config_dir
        .join(format!("{}.json", session.tunnel_id));
    if cred.exists() {
        if let Err(err) = fs::remove_file(&cred) {
            warn!("remove {}: {err}", cred.display());
        }
    }
}

fn delete_named_tunnel(
    cf: &dyn CloudflaredOps,
    config_dir: &Path,
    tunnel_name: &str,
    remove: bool,
    warn_only: bool,
) -> Result<(), Error> {
    let existing = match cf.list_tunnels() {
        Ok(rows) => rows,
        Err(err) if warn_only => {
            warn!("Could not check or delete tunnel: {err}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };
    let Some(row) = existing.into_iter().find(|row| row.name == tunnel_name) else {
        return Ok(());
    };
    if !remove {
        return Err(Error::TunnelAlreadyExists {
            name: tunnel_name.to_string(),
        });
    }
    info!("Removing existing tunnel: {tunnel_name}");
    if let Err(err) = cf.delete_tunnel(tunnel_name) {
        if warn_only {
            warn!("Could not check or delete tunnel: {err}");
            return Ok(());
        }
        return Err(err);
    }
    let cred = config_dir.join(format!("{}.json", row.id));
    if cred.exists() {
        let _ = fs::remove_file(cred);
    }
    Ok(())
}

pub fn named_run_command(session: &TunnelSession) -> String {
    format!(
        "{} tunnel run --url {} {}",
        session.binary, session.local, session.tunnel_name
    )
}

pub fn quick_run_command(binary: &str, local: &str) -> String {
    format!("{binary} tunnel --url {local}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::cloudflared::{MockCloudflared, TunnelRow};
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn home_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn with_temp_home<T>(f: impl FnOnce() -> T) -> T {
        let _g = home_lock();
        let dir = tempdir().unwrap();
        let prev = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path());
        let out = f();
        match prev {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        out
    }

    #[test]
    fn setup_creates_and_routes_with_overwrite() {
        with_temp_home(|| {
            let cf = MockCloudflared {
                has_cert: true,
                ..MockCloudflared::default()
            };
            let rt = TunnelRuntime::from_parts(cf);
            let session = setup_named(
                &rt,
                "api",
                "http://127.0.0.1:4000",
                "https://api.example.dev",
                true,
            )
            .unwrap();
            assert_eq!(session.tunnel_id, "id-api");
            assert_eq!(session.hostname, "api.example.dev");
            assert!(!session.config_dir.join("config.yml").exists());
            cleanup(&rt, &session);
        });
    }

    #[test]
    fn existing_tunnel_errors_without_remove_flag() {
        with_temp_home(|| {
            let cf = MockCloudflared {
                has_cert: true,
                list: Mutex::new(vec![TunnelRow {
                    id: "old".into(),
                    name: "api".into(),
                }]),
                ..MockCloudflared::default()
            };
            let rt = TunnelRuntime::from_parts(cf);
            let err = setup_named(
                &rt,
                "api",
                "http://127.0.0.1:4000",
                "https://api.example.dev",
                false,
            )
            .unwrap_err();
            assert!(matches!(err, Error::TunnelAlreadyExists { .. }));
        });
    }

    #[test]
    fn missing_cert_errors() {
        with_temp_home(|| {
            let cf = MockCloudflared {
                has_cert: false,
                ..MockCloudflared::default()
            };
            let rt = TunnelRuntime::from_parts(cf);
            let err = setup_named(
                &rt,
                "api",
                "http://127.0.0.1:4000",
                "https://api.example.dev",
                true,
            )
            .unwrap_err();
            assert!(matches!(err, Error::CloudflaredLoginRequired { .. }));
        });
    }

    #[test]
    fn run_lines() {
        let sess = TunnelSession {
            config_dir: PathBuf::from("/tmp"),
            tunnel_name: "api".into(),
            tunnel_id: "id".into(),
            hostname: "api.example.dev".into(),
            local: "http://127.0.0.1:4000".into(),
            binary: "cloudflared".into(),
        };
        assert_eq!(
            named_run_command(&sess),
            "cloudflared tunnel run --url http://127.0.0.1:4000 api"
        );
        assert_eq!(
            quick_run_command("cloudflared", "http://127.0.0.1:3000"),
            "cloudflared tunnel --url http://127.0.0.1:3000"
        );
    }
}
