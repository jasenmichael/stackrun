use crate::error::Error;
use serde::Deserialize;
use std::sync::Mutex;

pub trait DnsApi: Send + Sync {
    fn zone_id(&self, domain: &str, token: &str) -> Result<Option<String>, Error>;
    fn dns_record_id(
        &self,
        zone_id: &str,
        hostname: &str,
        token: &str,
    ) -> Result<Option<String>, Error>;
    fn delete_dns_record(&self, zone_id: &str, record_id: &str, token: &str) -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub struct RealDns {
    pub base_url: String,
}

impl Default for RealDns {
    fn default() -> Self {
        Self {
            base_url: std::env::var("STACKRUN_CF_API")
                .unwrap_or_else(|_| "https://api.cloudflare.com/client/v4".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    result: Option<Vec<IdRow>>,
    errors: Option<Vec<CfError>>,
    success: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct IdRow {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CfError {
    message: Option<String>,
}

impl RealDns {
    fn get_json(&self, url: &str, token: &str) -> Result<ListResponse, Error> {
        let resp = reqwest::blocking::Client::new()
            .get(url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .send()
            .map_err(|err| Error::CloudflareApi {
                message: err.to_string(),
            })?;
        let status = resp.status();
        let body: ListResponse = resp.json().map_err(|err| Error::CloudflareApi {
            message: err.to_string(),
        })?;
        if !status.is_success() || body.success == Some(false) {
            let msg = body
                .errors
                .unwrap_or_default()
                .into_iter()
                .filter_map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(Error::CloudflareApi {
                message: if msg.is_empty() {
                    status.to_string()
                } else {
                    msg
                },
            });
        }
        Ok(body)
    }
}

impl DnsApi for RealDns {
    fn zone_id(&self, domain: &str, token: &str) -> Result<Option<String>, Error> {
        let url = format!("{}/zones?name={domain}", self.base_url);
        let body = self.get_json(&url, token)?;
        Ok(body
            .result
            .unwrap_or_default()
            .into_iter()
            .find_map(|row| row.id))
    }

    fn dns_record_id(
        &self,
        zone_id: &str,
        hostname: &str,
        token: &str,
    ) -> Result<Option<String>, Error> {
        let url = format!(
            "{}/zones/{zone_id}/dns_records?name={hostname}",
            self.base_url
        );
        let body = self.get_json(&url, token)?;
        Ok(body
            .result
            .unwrap_or_default()
            .into_iter()
            .find_map(|row| row.id))
    }

    fn delete_dns_record(&self, zone_id: &str, record_id: &str, token: &str) -> Result<(), Error> {
        let url = format!("{}/zones/{zone_id}/dns_records/{record_id}", self.base_url);
        let resp = reqwest::blocking::Client::new()
            .delete(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .send()
            .map_err(|err| Error::CloudflareApi {
                message: err.to_string(),
            })?;
        if !resp.status().is_success() {
            return Err(Error::CloudflareApi {
                message: format!("DELETE DNS {}", resp.status()),
            });
        }
        Ok(())
    }
}

/// Hostname → registrable domain the same way cf-tunnel does (last two labels).
pub fn extract_domain(hostname: &str) -> &str {
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() >= 2 {
        let start =
            hostname.len() - parts[parts.len() - 2].len() - 1 - parts[parts.len() - 1].len();
        &hostname[start..]
    } else {
        hostname
    }
}

pub struct RecordingDns {
    pub zone_id: Option<String>,
    pub record_id: Option<String>,
    pub deletes: Mutex<Vec<(String, String)>>,
}

impl Default for RecordingDns {
    fn default() -> Self {
        Self {
            zone_id: Some("zone1".into()),
            record_id: None,
            deletes: Mutex::new(Vec::new()),
        }
    }
}

impl DnsApi for RecordingDns {
    fn zone_id(&self, _domain: &str, _token: &str) -> Result<Option<String>, Error> {
        Ok(self.zone_id.clone())
    }

    fn dns_record_id(
        &self,
        _zone_id: &str,
        _hostname: &str,
        _token: &str,
    ) -> Result<Option<String>, Error> {
        Ok(self.record_id.clone())
    }

    fn delete_dns_record(&self, zone_id: &str, record_id: &str, _token: &str) -> Result<(), Error> {
        self.deletes
            .lock()
            .unwrap()
            .push((zone_id.to_string(), record_id.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_from_hostname() {
        assert_eq!(extract_domain("api.example.dev"), "example.dev");
        assert_eq!(extract_domain("example.dev"), "example.dev");
        assert_eq!(
            extract_domain("api-dev.jasenmichael.com"),
            "jasenmichael.com"
        );
    }
}
