use crate::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelRow {
    pub id: String,
    pub name: String,
}

pub trait CloudflaredOps: Send + Sync {
    fn binary_path(&self) -> Result<String, Error>;
    fn has_cert(&self, config_dir: &Path) -> bool;
    fn list_tunnels(&self) -> Result<Vec<TunnelRow>, Error>;
    fn delete_tunnel(&self, id: &str) -> Result<(), Error>;
    fn create_tunnel(&self, name: &str) -> Result<(), Error>;
    fn route_dns(&self, tunnel_name: &str, hostname: &str) -> Result<(), Error>;
}

#[derive(Debug, Default)]
pub struct RealCloudflared;

impl RealCloudflared {
    fn bin() -> Result<String, Error> {
        which("cloudflared").ok_or(Error::CloudflaredMissing)
    }

    fn run_capture(&self, args: &[&str]) -> Result<String, Error> {
        let bin = Self::bin()?;
        let output = Command::new(&bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(Error::Cloudflared {
                message: format!("{} {stderr}{stdout}", args.join(" ")).trim().into(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl CloudflaredOps for RealCloudflared {
    fn binary_path(&self) -> Result<String, Error> {
        Self::bin()
    }

    fn has_cert(&self, config_dir: &Path) -> bool {
        config_dir.join("cert.pem").is_file()
    }

    fn list_tunnels(&self) -> Result<Vec<TunnelRow>, Error> {
        let stdout = self.run_capture(&["tunnel", "list"])?;
        Ok(parse_tunnel_list(&stdout))
    }

    fn delete_tunnel(&self, id: &str) -> Result<(), Error> {
        self.run_capture(&["tunnel", "delete", id]).map(|_| ())
    }

    fn create_tunnel(&self, name: &str) -> Result<(), Error> {
        self.run_capture(&["tunnel", "create", name]).map(|_| ())
    }

    fn route_dns(&self, tunnel_name: &str, hostname: &str) -> Result<(), Error> {
        self.run_capture(&["tunnel", "route", "dns", tunnel_name, hostname])
            .map(|_| ())
    }
}

pub fn which(name: &str) -> Option<String> {
    let exe = if cfg!(windows) && !name.ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(&exe);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

pub fn parse_tunnel_list(stdout: &str) -> Vec<TunnelRow> {
    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.to_ascii_uppercase().starts_with("ID") {
                return None;
            }
            let mut parts = line.split_whitespace();
            let id = parts.next()?.to_string();
            let name = parts.next()?.to_string();
            Some(TunnelRow { id, name })
        })
        .collect()
}

pub fn default_config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(windows) {
        home.join("AppData").join("Local").join("cloudflared")
    } else if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("cloudflared")
    } else {
        home.join(".cloudflared")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_rows() {
        let out = "ID                                   NAME     CREATED\n\
                   abc-123                               api      2024-01-01\n";
        let rows = parse_tunnel_list(out);
        assert_eq!(rows[0].id, "abc-123");
        assert_eq!(rows[0].name, "api");
    }
}
