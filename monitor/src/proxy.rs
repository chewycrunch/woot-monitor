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
    pub fn to_proxy_url(&self) -> String {
        let mut url = format!("http://{}:{}", self.host, self.port);
        if let (Some(ref user), Some(ref pass)) = (&self.user, &self.pass) {
            url = format!("http://{}:{}@{}", user, pass, url);
        }
        url
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
