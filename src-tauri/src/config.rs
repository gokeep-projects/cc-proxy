use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_log_capacity")]
    pub log_capacity: usize,
    #[serde(default)]
    pub https: bool,
    #[serde(default)]
    pub cert_path: String,
    #[serde(default)]
    pub key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    pub from: String,
    pub to_provider: String,
    pub to_model: String,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    9528
}
fn default_log_capacity() -> usize {
    10000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig {
                host: default_host(),
                port: default_port(),
                log_capacity: default_log_capacity(),
                https: false,
                cert_path: String::new(),
                key_path: String::new(),
            },
            providers: vec![],
            model_mappings: vec![],
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, content).map_err(|e| e.to_string())
    }

    pub fn find_mapping(&self, model: &str) -> Option<&ModelMapping> {
        self.model_mappings.iter().find(|m| m.from == model)
    }

    pub fn find_provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }
}
