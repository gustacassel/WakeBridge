use crate::processo::sem_janela;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::process::Stdio;
use std::time::Duration;

/// Roda na ponte: 6 bytes FF + MAC 16 vezes, em broadcast geral e da sub-rede local
const SCRIPT: &str = r#"import socket
mac = bytes.fromhex('{MAC}')
pacote = b'\xff' * 6 + mac * 16
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
destinos = ['255.255.255.255']
try:
    t = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    t.connect(('8.8.8.8', 80))
    destinos.append(t.getsockname()[0].rsplit('.', 1)[0] + '.255')
except OSError:
    pass
for d in destinos:
    s.sendto(pacote, (d, 9))
print('enviado para ' + ', '.join(destinos))
"#;

/// "AA:BB:CC:11:22:33" -> "AABBCC112233"
pub fn normalizar_mac(mac: &str) -> Result<String, String> {
    let hex: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 12 && mac.chars().all(|c| c.is_ascii_hexdigit() || ":-. ".contains(c)) {
        Ok(hex.to_uppercase())
    } else {
        Err(format!("MAC inválido: {mac}"))
    }
}

/// Entra por SSH na ponte e manda o script pela entrada padrão (nada precisa existir lá além do python)
pub fn enviar_pela_ponte(ip_ponte: &str, porta: u16, mac: &str) -> Result<String, String> {
    let mac = normalizar_mac(mac)?;
    let mut ssh = sem_janela("ssh")
        .args([
            "-p", &porta.to_string(),
            // Sem terminal para digitar senha: só funciona com chave SSH
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=10",
            "-o", "StrictHostKeyChecking=accept-new",
            ip_ponte,
            "python", "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Não consegui rodar o ssh: {e}"))?;

    ssh.stdin
        .take()
        .ok_or("Sem stdin do ssh")?
        .write_all(SCRIPT.replace("{MAC}", &mac).as_bytes())
        .map_err(|e| e.to_string())?;

    let saida = ssh.wait_with_output().map_err(|e| e.to_string())?;
    if saida.status.success() {
        Ok(String::from_utf8_lossy(&saida.stdout).trim().to_string())
    } else {
        let erro = String::from_utf8_lossy(&saida.stderr).trim().to_string();
        let dica = if erro.contains("Permission denied") {
            " A chave SSH deste computador está no authorized_keys da ponte?"
        } else if erro.contains("timed out") || erro.contains("refused") {
            " O sshd do Termux está rodando na ponte?"
        } else {
            ""
        };
        Err(format!("A ponte não respondeu: {erro}.{dica}"))
    }
}

pub fn rdp_aberto(ip: &str) -> bool {
    match format!("{ip}:3389").parse::<SocketAddr>() {
        Ok(endereco) => TcpStream::connect_timeout(&endereco, Duration::from_secs(2)).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aceita_mac_em_varios_formatos() {
        for mac in ["AA:BB:CC:11:22:33", "aa-bb-cc-11-22-33", "AABBCC112233"] {
            assert_eq!(normalizar_mac(mac).unwrap(), "AABBCC112233");
        }
    }

    #[test]
    fn recusa_mac_invalido() {
        for mac in ["", "AA:BB:CC:11:22", "AA:BB:CC:11:22:33:44", "GA:BB:CC:11:22:33", "AA;BB;CC;11;22;33"] {
            assert!(normalizar_mac(mac).is_err(), "{mac} deveria ser inválido");
        }
    }

    #[test]
    fn script_recebe_o_mac() {
        let script = SCRIPT.replace("{MAC}", "AABBCC112233");
        assert!(script.contains("bytes.fromhex('AABBCC112233')"));
        assert!(script.contains(r"b'\xff' * 6"));
    }
}
