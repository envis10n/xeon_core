use crate::fs::read_file;
use serde::Deserialize;
use std::path::Path;
use tokio::io::Error;
use toml;

#[derive(Deserialize)]
pub struct WebSocketConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl WebSocketConfig {
    pub fn bind_addr(&self) -> String {
        let host: &str;
        if let Some(h) = &self.host {
            host = h;
        } else {
            host = "127.0.0.1"
        }
        let port: u16;
        if let Some(p) = self.port {
            port = p;
        } else {
            port = 3500;
        }
        format!("{}:{}", host, port)
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        WebSocketConfig {
            host: Some("127.0.0.1".to_string()),
            port: Some(3500),
        }
    }
}

#[derive(Deserialize)]
pub struct XeonConfig {
    pub mongodb_str: Option<String>,
    pub websocket: Option<WebSocketConfig>,
}

pub async fn read_config_file(p: impl AsRef<Path>) -> Result<XeonConfig, Error> {
    let buf = read_file(p).await?;
    let c: XeonConfig = toml::from_slice(&buf)?;
    Ok(c)
}
