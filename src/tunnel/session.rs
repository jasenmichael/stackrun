use super::cloudflared::{default_config_dir, CloudflaredOps, TunnelRow};
use super::dns::{extract_domain, DnsApi};
use super::{Ingress, TunnelRuntime};
use crate::config::types::CfTunnelConfig;
use crate::error::Error;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug)]
pub struct TunnelSession {
    pub config_dir: PathBuf,
    pub tunnel_name: String,
    pub tunnel_id: String,
    pub ingress: Vec<Ingress>,
    pub token: String,
    pub binary: String,
    pub run_cwd: Option<String>,
    pub run_name: String,
    pub run_color: Option<String>,
}

#[derive(Serialize)]
struct CloudflaredFile {
    tunnel: String,
    #[serde(rename = "credentials-file")]
    credentials_file: String,
    ingress: Vec<serde_yaml::Value>,
}

pub fn setup(
    runtime: &TunnelRuntime,
    cfg: Option<&CfTunnelConfig>,
    ingress: Vec<Ingress>,
    token: String,
) -> Result<TunnelSession, Error> {
    let tunnel_name = super::resolve_tunnel_name(cfg);
    let config_dir = cfg
        .and_then(|c| c.cloudflared_config_dir.clone())
        .map(PathBuf::from)
        .unwrap_or_else(default_config_dir);
    fs::create_dir_all(&config_dir)?;

    let binary = runtime.cloudflared.binary_path()?;
    if !runtime.cloudflared.has_cert(&config_dir) {
        return Err(Error::CloudflaredLoginRequired {
            dir: config_dir.display().to_string(),
        });
    }

    let remove_tunnel = cfg.and_then(|c| c.remove_existing_tunnel).unwrap_or(false);
    let remove_dns = cfg.and_then(|c| c.remove_existing_dns).unwrap_or(false);

    delete_named_tunnel(
        runtime.cloudflared.as_ref(),
        &config_dir,
        &tunnel_name,
        remove_tunnel,
        false,
    )?;
    delete_dns_records(runtime.dns.as_ref(), &ingress, &token, remove_dns, false)?;

    runtime.cloudflared.create_tunnel(&tunnel_name)?;
    for row in &ingress {
        runtime.cloudflared.route_dns(&tunnel_name, &row.hostname)?;
    }

    let tunnel_id = runtime
        .cloudflared
        .list_tunnels()?
        .into_iter()
        .find(|row| row.name == tunnel_name)
        .map(|row| row.id)
        .ok_or_else(|| Error::Cloudflared {
            message: format!(
                "created tunnel `{tunnel_name}` but it did not appear in `cloudflared tunnel list`"
            ),
        })?;

    write_config_yml(&config_dir, &tunnel_name, &tunnel_id, &ingress)?;

    let opts = cfg.and_then(|c| c.command_options.as_ref());
    let run_name = opts
        .and_then(|o| o.name.clone())
        .unwrap_or_else(|| "Tunnel".into());
    let run_color = opts
        .and_then(|o| o.prefix_color.clone())
        .or_else(|| Some("cyan".into()));

    Ok(TunnelSession {
        config_dir,
        tunnel_name,
        tunnel_id,
        ingress,
        token,
        binary,
        run_cwd: opts.and_then(|o| o.cwd.clone()),
        run_name,
        run_color,
    })
}

pub fn cleanup(runtime: &TunnelRuntime, session: &TunnelSession) {
    info!("Cleaning up tunnel resources...");
    if let Err(err) = delete_named_tunnel(
        runtime.cloudflared.as_ref(),
        &session.config_dir,
        &session.tunnel_name,
        true,
        true,
    ) {
        warn!("tunnel delete: {err}");
    }
    if let Err(err) = delete_dns_records(
        runtime.dns.as_ref(),
        &session.ingress,
        &session.token,
        true,
        true,
    ) {
        warn!("dns delete: {err}");
    }
    let config_yml = session.config_dir.join("config.yml");
    if config_yml.exists() {
        if let Err(err) = fs::remove_file(&config_yml) {
            warn!("remove {}: {err}", config_yml.display());
        }
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
    let Some(TunnelRow { id, .. }) = existing.into_iter().find(|row| row.name == tunnel_name)
    else {
        return Ok(());
    };
    if !remove {
        return Err(Error::TunnelAlreadyExists {
            name: tunnel_name.to_string(),
        });
    }
    info!("Removing existing tunnel: {tunnel_name}");
    if let Err(err) = cf.delete_tunnel(&id) {
        if warn_only {
            warn!("Could not check or delete tunnel: {err}");
            return Ok(());
        }
        return Err(err);
    }
    let cred = config_dir.join(format!("{id}.json"));
    if cred.exists() {
        let _ = fs::remove_file(cred);
    }
    Ok(())
}

fn delete_dns_records(
    dns: &dyn DnsApi,
    ingress: &[Ingress],
    token: &str,
    remove: bool,
    warn_only: bool,
) -> Result<(), Error> {
    for row in ingress {
        let domain = extract_domain(&row.hostname);
        let zone_id = match dns.zone_id(domain, token) {
            Ok(id) => id,
            Err(err) if warn_only => {
                warn!("Error getting zone ID for {domain}: {err}");
                continue;
            }
            Err(err) => return Err(err),
        };
        let Some(zone_id) = zone_id else {
            warn!("Zone ID not found for domain: {domain}");
            continue;
        };
        let record_id = match dns.dns_record_id(&zone_id, &row.hostname, token) {
            Ok(id) => id,
            Err(err) if warn_only => {
                warn!("dns lookup {}: {err}", row.hostname);
                continue;
            }
            Err(err) => return Err(err),
        };
        let Some(record_id) = record_id else {
            continue;
        };
        if !remove {
            return Err(Error::DnsRecordExists {
                hostname: row.hostname.clone(),
            });
        }
        info!("Removing existing DNS record: {}", row.hostname);
        if let Err(err) = dns.delete_dns_record(&zone_id, &record_id, token) {
            if warn_only {
                warn!("dns delete {}: {err}", row.hostname);
                continue;
            }
            return Err(err);
        }
    }
    Ok(())
}

fn write_config_yml(
    config_dir: &Path,
    tunnel_name: &str,
    tunnel_id: &str,
    ingress: &[Ingress],
) -> Result<(), Error> {
    let mut entries: Vec<serde_yaml::Value> = ingress
        .iter()
        .map(|row| {
            serde_yaml::to_value(serde_json::json!({
                "hostname": row.hostname,
                "service": row.service,
            }))
            .expect("ingress yaml")
        })
        .collect();
    entries.push(
        serde_yaml::to_value(serde_json::json!({ "service": "http_status:404" }))
            .expect("catch-all yaml"),
    );
    let file = CloudflaredFile {
        tunnel: tunnel_name.to_string(),
        credentials_file: config_dir
            .join(format!("{tunnel_id}.json"))
            .to_string_lossy()
            .into_owned(),
        ingress: entries,
    };
    let yaml = serde_yaml::to_string(&file).map_err(|err| Error::Message(err.to_string()))?;
    fs::write(config_dir.join("config.yml"), yaml)?;
    Ok(())
}

pub fn run_command_line(session: &TunnelSession) -> String {
    format!("{} tunnel run {}", session.binary, session.tunnel_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel::cloudflared::CloudflaredOps;
    use crate::tunnel::dns::RecordingDns;
    use crate::tunnel::Ingress;
    use std::sync::Mutex;
    use tempfile::tempdir;

    #[derive(Default)]
    struct MockCf {
        has_cert: bool,
        list: Mutex<Vec<TunnelRow>>,
        created: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
        routed: Mutex<Vec<(String, String)>>,
        fail_list: bool,
    }

    impl CloudflaredOps for MockCf {
        fn binary_path(&self) -> Result<String, Error> {
            Ok("cloudflared".into())
        }
        fn has_cert(&self, _config_dir: &Path) -> bool {
            self.has_cert
        }
        fn list_tunnels(&self) -> Result<Vec<TunnelRow>, Error> {
            if self.fail_list {
                return Err(Error::Cloudflared {
                    message: "list failed".into(),
                });
            }
            Ok(self.list.lock().unwrap().clone())
        }
        fn delete_tunnel(&self, id: &str) -> Result<(), Error> {
            self.deleted.lock().unwrap().push(id.to_string());
            self.list.lock().unwrap().retain(|r| r.id != id);
            Ok(())
        }
        fn create_tunnel(&self, name: &str) -> Result<(), Error> {
            self.created.lock().unwrap().push(name.to_string());
            self.list.lock().unwrap().push(TunnelRow {
                id: "new-id".into(),
                name: name.to_string(),
            });
            Ok(())
        }
        fn route_dns(&self, tunnel_name: &str, hostname: &str) -> Result<(), Error> {
            self.routed
                .lock()
                .unwrap()
                .push((tunnel_name.to_string(), hostname.to_string()));
            Ok(())
        }
    }

    fn ingress() -> Vec<Ingress> {
        vec![Ingress {
            hostname: "api.example.dev".into(),
            service: "http://localhost:4000".into(),
        }]
    }

    #[test]
    fn setup_creates_and_routes() {
        let dir = tempdir().unwrap();
        let cf = MockCf {
            has_cert: true,
            ..MockCf::default()
        };
        let dns = RecordingDns::default();
        let rt = TunnelRuntime::from_parts(cf, dns);
        let cfg = CfTunnelConfig {
            cloudflared_config_dir: Some(dir.path().display().to_string()),
            remove_existing_tunnel: Some(true),
            remove_existing_dns: Some(true),
            ..CfTunnelConfig::default()
        };
        let session = setup(&rt, Some(&cfg), ingress(), "token".into()).unwrap();
        assert_eq!(session.tunnel_id, "new-id");
        assert!(dir.path().join("config.yml").exists());
        cleanup(&rt, &session);
        assert!(!dir.path().join("config.yml").exists());
    }

    #[test]
    fn existing_tunnel_errors_without_remove_flag() {
        let dir = tempdir().unwrap();
        let cf = MockCf {
            has_cert: true,
            list: Mutex::new(vec![TunnelRow {
                id: "old".into(),
                name: "stackrun".into(),
            }]),
            ..MockCf::default()
        };
        let rt = TunnelRuntime::from_parts(cf, RecordingDns::default());
        let cfg = CfTunnelConfig {
            cloudflared_config_dir: Some(dir.path().display().to_string()),
            remove_existing_tunnel: Some(false),
            ..CfTunnelConfig::default()
        };
        let err = setup(&rt, Some(&cfg), ingress(), "token".into()).unwrap_err();
        assert!(matches!(err, Error::TunnelAlreadyExists { .. }));
    }

    #[test]
    fn existing_dns_errors_without_remove_flag() {
        let dir = tempdir().unwrap();
        let cf = MockCf {
            has_cert: true,
            ..MockCf::default()
        };
        let dns = RecordingDns {
            record_id: Some("rec1".into()),
            ..RecordingDns::default()
        };
        let rt = TunnelRuntime::from_parts(cf, dns);
        let cfg = CfTunnelConfig {
            cloudflared_config_dir: Some(dir.path().display().to_string()),
            remove_existing_dns: Some(false),
            ..CfTunnelConfig::default()
        };
        let err = setup(&rt, Some(&cfg), ingress(), "token".into()).unwrap_err();
        assert!(matches!(err, Error::DnsRecordExists { .. }));
    }

    #[test]
    fn missing_cert_errors() {
        let dir = tempdir().unwrap();
        let cf = MockCf {
            has_cert: false,
            ..MockCf::default()
        };
        let rt = TunnelRuntime::from_parts(cf, RecordingDns::default());
        let cfg = CfTunnelConfig {
            cloudflared_config_dir: Some(dir.path().display().to_string()),
            ..CfTunnelConfig::default()
        };
        let err = setup(&rt, Some(&cfg), ingress(), "token".into()).unwrap_err();
        assert!(matches!(err, Error::CloudflaredLoginRequired { .. }));
    }
}
