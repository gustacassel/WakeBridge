use crate::processo::sem_janela;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const TAILSCALE_PADRAO: &str = r"C:\Program Files\Tailscale\tailscale.exe";

/// Um nó como aparece no `tailscale status --json` (só os campos que usamos)
#[derive(Deserialize)]
struct No {
    #[serde(rename = "HostName", default)]
    host_name: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "OS", default)]
    os: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Option<Vec<String>>,
    #[serde(rename = "Online", default)]
    online: bool,
}

#[derive(Deserialize)]
struct Status {
    #[serde(rename = "BackendState", default)]
    backend_state: String,
    #[serde(rename = "Self")]
    eu: Option<No>,
    #[serde(rename = "Peer", default)]
    peer: Option<HashMap<String, No>>,
}

#[derive(Serialize, Clone)]
pub struct Dispositivo {
    /// Primeiro rótulo do nome MagicDNS, ex.: "meu-pc"
    pub id: String,
    pub nome: String,
    pub sistema: String,
    pub ip: Option<String>,
    pub online: bool,
    /// É a máquina onde o app está rodando
    pub este: bool,
}

impl No {
    fn para_dispositivo(self, este: bool) -> Dispositivo {
        let id = match self.dns_name.split('.').next() {
            Some(rotulo) if !rotulo.is_empty() => rotulo.to_string(),
            _ => self.host_name.to_lowercase().replace(' ', "-"),
        };
        let ip = self
            .tailscale_ips
            .unwrap_or_default()
            .into_iter()
            .find(|ip| ip.contains('.'));
        Dispositivo {
            id,
            nome: self.host_name,
            sistema: self.os,
            ip,
            online: este || self.online,
            este,
        }
    }
}

fn executavel() -> &'static str {
    if Path::new(TAILSCALE_PADRAO).exists() { TAILSCALE_PADRAO } else { "tailscale" }
}

pub fn listar() -> Result<Vec<Dispositivo>, String> {
    let saida = sem_janela(executavel())
        .args(["status", "--json"])
        .output()
        .map_err(|e| format!("Não achei o Tailscale instalado: {e}"))?;

    let status: Status = serde_json::from_slice(&saida.stdout)
        .map_err(|e| format!("Resposta inesperada do Tailscale: {e}"))?;

    if status.backend_state != "Running" {
        return Err(format!(
            "O Tailscale não está conectado (estado: {}). Abra o app do Tailscale.",
            status.backend_state
        ));
    }

    let mut lista: Vec<Dispositivo> = status.eu.into_iter().map(|n| n.para_dispositivo(true)).collect();
    let mut outros: Vec<Dispositivo> = status
        .peer
        .unwrap_or_default()
        .into_values()
        .map(|n| n.para_dispositivo(false))
        .collect();
    outros.sort_by(|a, b| b.online.cmp(&a.online).then(a.nome.to_lowercase().cmp(&b.nome.to_lowercase())));
    lista.extend(outros);
    Ok(lista)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn id_vem_do_magicdns_e_ip_e_o_v4() {
        let no: No = serde_json::from_str(
            r#"{"HostName":"Old Phone","DNSName":"old-phone.tail1234.ts.net.","OS":"android",
                "TailscaleIPs":["100.64.0.2","fd7a:115c:a1e0::1"],"Online":true}"#,
        )
        .unwrap();
        let d = no.para_dispositivo(false);
        assert_eq!(d.id, "old-phone");
        assert_eq!(d.nome, "Old Phone");
        assert_eq!(d.ip.as_deref(), Some("100.64.0.2"));
        assert!(d.online);
    }

    #[test]
    fn sem_magicdns_usa_o_hostname() {
        let no: No = serde_json::from_str(r#"{"HostName":"Meu PC","DNSName":"","OS":"windows"}"#).unwrap();
        let d = no.para_dispositivo(false);
        assert_eq!(d.id, "meu-pc");
        assert_eq!(d.ip, None);
        assert!(!d.online);
    }

    /// Lê o Tailscale de verdade: cargo test -- --ignored --nocapture
    #[test]
    #[ignore]
    fn lista_a_rede_real() {
        for d in listar().unwrap() {
            println!("{:<20} {:<10} {:<16} online={} este={}", d.id, d.sistema, d.ip.unwrap_or_default(), d.online, d.este);
        }
    }
}
