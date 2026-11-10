// Shared outbound HTTP guard for user or source controlled URLs.
// HTTPS only, public addresses only, DNS pinned per connection, every redirect
// re-checked, and response bodies read through a streaming byte cap.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::LOCATION;
use reqwest::{Client, Response, Url};

pub(crate) const MAX_REDIRECTS: usize = 5;
const DNS_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

pub type LoopbackAuthorizer = Arc<dyn Fn(&Url) -> bool + Send + Sync>;

/// A reqwest client with a resolver that rejects non-public addresses and
/// returns only the addresses it just checked. That pins each connection to a
/// validated answer, so a second DNS answer cannot rebind the request.
#[derive(Clone)]
pub struct SafeFetch {
    client: Client,
    loopback_client: Client,
    allow_loopback: Option<LoopbackAuthorizer>,
    max_redirects: usize,
}

impl SafeFetch {
    pub fn new(user_agent: &str, timeout: Option<Duration>, no_gzip: bool) -> Result<Self, String> {
        Self::build(user_agent, timeout, no_gzip, None)
    }

    /// Builds a client that normally requires HTTPS and public addresses, but
    /// accepts a loopback URL only when `allow_loopback` returns true. The
    /// downloader uses this for the app's own stream proxy, whose URLs are
    /// bound to an active entry and a random token.
    pub fn new_with_loopback_authorizer(
        user_agent: &str,
        timeout: Option<Duration>,
        no_gzip: bool,
        allow_loopback: LoopbackAuthorizer,
    ) -> Result<Self, String> {
        Self::build(user_agent, timeout, no_gzip, Some(allow_loopback))
    }

    fn build(
        user_agent: &str,
        timeout: Option<Duration>,
        no_gzip: bool,
        allow_loopback: Option<LoopbackAuthorizer>,
    ) -> Result<Self, String> {
        let mut builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(Duration::from_secs(60))
            .dns_resolver(Arc::new(PublicResolver))
            .no_proxy()
            .user_agent(user_agent.to_string());
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }
        if no_gzip {
            builder = builder.no_gzip();
        }
        let client = builder
            .build()
            .map_err(|err| format!("network client failed: {err}"))?;

        let mut loopback_builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(Duration::from_secs(60))
            .no_proxy()
            .user_agent(user_agent.to_string());
        if let Some(timeout) = timeout {
            loopback_builder = loopback_builder.timeout(timeout);
        }
        if no_gzip {
            loopback_builder = loopback_builder.no_gzip();
        }
        let loopback_client = loopback_builder
            .build()
            .map_err(|err| format!("network client failed: {err}"))?;

        Ok(Self {
            client,
            loopback_client,
            allow_loopback,
            max_redirects: MAX_REDIRECTS,
        })
    }

    /// Sends a GET and follows up to `MAX_REDIRECTS` redirects. The caller's
    /// builder is recreated for every hop so headers can be filtered per origin.
    pub async fn get<F>(&self, url: &str, prepare: F) -> Result<Response, String>
    where
        F: Fn(&Url, reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        let mut current = Url::parse(url.trim()).map_err(|_| "URL is not valid".to_string())?;
        for hop in 0..=self.max_redirects {
            let loopback = self
                .allow_loopback
                .as_ref()
                .is_some_and(|allow| allow(&current));
            if loopback {
                validate_loopback_url(&current)?;
            } else {
                validate_https_url(&current)?;
            }
            let client = if loopback {
                &self.loopback_client
            } else {
                &self.client
            };
            let response = prepare(&current, client.get(current.clone()))
                .send()
                .await
                .map_err(network_error)?;
            if !response.status().is_redirection() {
                return Ok(response);
            }
            if hop == self.max_redirects {
                return Err("too many redirects".to_string());
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "redirect response has no valid Location".to_string())?;
            let next = current
                .join(location)
                .map_err(|_| "redirect target is not a valid URL".to_string())?;
            current = next;
        }
        Err("too many redirects".to_string())
    }
}

struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let lookup =
                tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host.as_str(), 0)))
                    .await
                    .map_err(|_| blocked("DNS lookup timed out"))?
                    .map_err(|_| blocked("host could not be resolved"))?;
            let mut addresses = Vec::new();
            for address in lookup {
                if !is_public_ip(address.ip()) {
                    return Err(blocked("host resolves to a non-public address"));
                }
                addresses.push(address);
            }
            if addresses.is_empty() {
                return Err(blocked("host has no addresses"));
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

fn blocked(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        message.to_string(),
    ))
}

fn network_error(error: reqwest::Error) -> String {
    if error.is_redirect() {
        "network redirect was refused".to_string()
    } else if error.is_timeout() {
        "network request timed out".to_string()
    } else if error.is_connect() {
        "network host could not be reached".to_string()
    } else {
        "network request failed".to_string()
    }
}

/// Accepts only an absolute HTTPS URL with a host, no credentials and no
/// local/private literal address. Host names are checked again by the pinned
/// resolver when the connection opens.
pub fn validate_https_url(url: &Url) -> Result<(), String> {
    if url.scheme() != "https" {
        return Err("only https URLs are allowed".to_string());
    }
    if url.host_str().is_none() {
        return Err("URL has no host".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL must not contain credentials".to_string());
    }

    let host = url
        .host_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err("URL has no host".to_string());
    }
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return Err("URL host is local or private".to_string());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            return Err("URL address is not public".to_string());
        }
    } else {
        let bare = host
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .unwrap_or(&host);
        if let Ok(ip) = bare.parse::<IpAddr>() {
            if !is_public_ip(ip) {
                return Err("URL address is not public".to_string());
            }
        }
    }
    Ok(())
}

/// The only loopback shape accepted is the app's own proxy on 127.0.0.1. The
/// caller-supplied authorizer must also prove the entry and token belong to a
/// live proxy registration before this path is used.
fn validate_loopback_url(url: &Url) -> Result<(), String> {
    if url.scheme() != "http" {
        return Err("the app proxy URL must use http".to_string());
    }
    if url.host_str() != Some("127.0.0.1") {
        return Err("the app proxy host must be 127.0.0.1".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("the app proxy URL must not contain credentials".to_string());
    }
    if !url.path().starts_with("/s/") {
        return Err("the app proxy URL has an invalid path".to_string());
    }
    Ok(())
}

pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || octets == [169, 254, 169, 254]
                || octets[0] == 0
                || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
                || octets[0] >= 240)
        }
        IpAddr::V6(ip) => {
            !(ip.to_ipv4_mapped()
                .is_some_and(|ip| !is_public_ip(IpAddr::V4(ip)))
                || ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast())
        }
    }
}

/// Reads a response body while enforcing `cap` on every chunk, so a server
/// cannot allocate a giant body before the size check.
pub async fn read_capped(
    mut response: Response,
    cap: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > cap as u64)
    {
        return Err(format!("{label} is larger than the {cap}-byte limit"));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| format!("{label} download failed"))?
    {
        if body.len().saturating_add(chunk.len()) > cap {
            return Err(format!("{label} is larger than the {cap}-byte limit"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn accepts_public_https_urls() {
        assert!(validate_https_url(&parse("https://example.com/index.json")).is_ok());
        assert!(validate_https_url(&parse("https://8.8.8.8/file.js")).is_ok());
        assert!(validate_https_url(&parse("https://[2606:4700:4700::1111]/file.js")).is_ok());
    }

    #[test]
    fn rejects_plain_http_and_other_schemes() {
        assert!(validate_https_url(&parse("http://example.com/index.json")).is_err());
        assert!(validate_https_url(&parse("file:///C:/Windows/win.ini")).is_err());
        assert!(validate_https_url(&parse("ftp://example.com/file")).is_err());
    }

    #[test]
    fn rejects_credentials_and_local_names() {
        assert!(validate_https_url(&parse("https://user:pass@example.com/file")).is_err());
        assert!(validate_https_url(&parse("https://user@example.com/file")).is_err());
        assert!(validate_https_url(&parse("https://localhost/file")).is_err());
        assert!(validate_https_url(&parse("https://printer.local/file")).is_err());
        assert!(validate_https_url(&parse("https://service.internal/file")).is_err());
    }

    #[test]
    fn rejects_non_public_literal_addresses() {
        for value in [
            "https://127.0.0.1/",
            "https://10.1.2.3/",
            "https://172.16.0.1/",
            "https://192.168.1.1/",
            "https://169.254.169.254/",
            "https://0.0.0.0/",
            "https://[::1]/",
            "https://[fc00::1]/",
            "https://[fe80::1]/",
        ] {
            assert!(validate_https_url(&parse(value)).is_err(), "{value}");
        }
    }

    #[test]
    fn classifies_public_and_private_addresses() {
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_public_ip("192.168.0.1".parse().unwrap()));
        assert!(!is_public_ip("100.64.0.1".parse().unwrap()));
        assert!(!is_public_ip("198.18.0.1".parse().unwrap()));
        assert!(!is_public_ip("240.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(!is_public_ip("::ffff:127.0.0.1".parse().unwrap()));
    }
}
