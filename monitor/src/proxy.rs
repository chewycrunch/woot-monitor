use std::fs::{self};
use std::sync::RwLock;

#[derive(Clone)]
pub struct Proxy {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub pass: Option<String>,
}

impl Proxy {
    // @spec FETCHING-015
    /// Renders the proxy as a URL for the sidecar's `proxyUrl` field, with the
    /// credentials between the scheme and the host so the scheme appears once.
    pub fn to_proxy_url(&self) -> String {
        match (&self.user, &self.pass) {
            (Some(user), Some(pass)) => {
                format!("http://{}:{}@{}:{}", user, pass, self.host, self.port)
            }
            _ => format!("http://{}:{}", self.host, self.port),
        }
    }
    pub fn to_reqwest_proxy(&self) -> Option<reqwest::Proxy> {
        let url = format!("http://{}:{}", self.host, self.port);
        let mut proxy = reqwest::Proxy::all(&url).ok()?;

        if let (Some(ref user), Some(ref pass)) = (&self.user, &self.pass) {
            proxy = proxy.basic_auth(user, pass);
        }

        Some(proxy)
    }
}

pub struct ProxyManager {
    proxies: Vec<Proxy>,
    index: RwLock<usize>,
    filename: Option<String>,
}

impl ProxyManager {
    // @spec FETCHING-001, FETCHING-002, FETCHING-003
    pub fn new_from_file(path: &str) -> Self {
        let contents = fs::read_to_string(path).unwrap_or_default();

        let proxies = contents
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(':').collect();
                match parts.len() {
                    2 => Some(Proxy {
                        host: parts[0].to_string(),
                        port: parts[1].parse().ok()?,
                        user: None,
                        pass: None,
                    }),
                    4 => Some(Proxy {
                        host: parts[0].to_string(),
                        port: parts[1].parse().ok()?,
                        user: Some(parts[2].to_string()),
                        pass: Some(parts[3].to_string()),
                    }),
                    _ => None,
                }
            })
            .collect();

        Self {
            proxies,
            index: RwLock::new(0),
            filename: std::path::Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .map(String::from),
        }
    }

    // @spec FETCHING-005
    /// Returns the next proxy in round-robin fashion.
    pub fn get_next_proxy(&self) -> Option<Proxy> {
        let mut index = self.index.write().ok()?;
        if self.proxies.is_empty() {
            return None;
        }

        let proxy = self.proxies.get(*index)?.clone();
        *index = (*index + 1) % self.proxies.len();
        Some(proxy)
    }

    pub fn count(&self) -> usize {
        self.proxies.len()
    }

    pub fn filename(&self) -> Option<String> {
        self.filename.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(user: Option<&str>, pass: Option<&str>) -> Proxy {
        Proxy {
            host: "45.3.38.37".to_string(),
            port: 3129,
            user: user.map(String::from),
            pass: pass.map(String::from),
        }
    }

    // @spec FETCHING-015
    #[test]
    fn puts_credentials_between_the_scheme_and_the_host() {
        assert_eq!(
            proxy(Some("user"), Some("pass")).to_proxy_url(),
            "http://user:pass@45.3.38.37:3129"
        );
    }

    // @spec FETCHING-015
    #[test]
    fn omits_the_userinfo_when_there_are_no_credentials() {
        assert_eq!(proxy(None, None).to_proxy_url(), "http://45.3.38.37:3129");
    }

    // @spec FETCHING-015
    /// The sidecar takes the string verbatim, so a duplicate scheme is silent.
    #[test]
    fn writes_the_scheme_exactly_once() {
        assert_eq!(
            proxy(Some("user"), Some("pass"))
                .to_proxy_url()
                .matches("http://")
                .count(),
            1
        );
    }
}
