use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// O que o Tailscale não sabe: quem é a ponte e o MAC de cada aparelho.
/// Fica em %APPDATA%\com.gustacassel.wakebridge\config.json
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Id (nome MagicDNS) do aparelho que fica em casa e envia o pacote mágico
    pub ponte: Option<String>,
    /// Porta do sshd na ponte (8022 no Termux)
    pub porta_ssh: u16,
    /// Id do aparelho -> MAC da placa que acorda
    pub macs: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { ponte: None, porta_ssh: 8022, macs: HashMap::new() }
    }
}

fn caminho(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

pub fn ler(app: &AppHandle) -> Config {
    caminho(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn salvar(app: &AppHandle, config: &Config) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(caminho(app)?, json).map_err(|e| e.to_string())
}
