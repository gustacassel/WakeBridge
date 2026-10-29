mod acordar;
mod config;
mod processo;
mod tailscale;

use config::Config;
use std::process::Command;
use tailscale::Dispositivo;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

// Comandos async rodam fora da thread da interface, então ssh e tailscale não travam a janela

#[tauri::command]
async fn listar_dispositivos() -> Result<Vec<Dispositivo>, String> {
    tailscale::listar()
}

#[tauri::command]
fn ler_config(app: AppHandle) -> Config {
    config::ler(&app)
}

#[tauri::command]
fn salvar_config(app: AppHandle, config: Config) -> Result<(), String> {
    for mac in config.macs.values() {
        acordar::normalizar_mac(mac)?;
    }
    config::salvar(&app, &config)
}

#[tauri::command]
async fn rdp_aberto(ip: String) -> bool {
    acordar::rdp_aberto(&ip)
}

#[tauri::command]
fn conectar_rdp(ip: String) -> Result<(), String> {
    Command::new("mstsc")
        .arg(format!("/v:{ip}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Não consegui abrir o RDP: {e}"))
}

#[tauri::command]
async fn acordar(app: AppHandle, id: String) -> Result<String, String> {
    let config = config::ler(&app);
    let mac = config.macs.get(&id).ok_or("Esse aparelho não tem MAC configurado.")?;
    let id_ponte = config.ponte.as_ref().ok_or("Escolha qual aparelho é a ponte.")?;

    let dispositivos = tailscale::listar()?;
    let ponte = dispositivos
        .iter()
        .find(|d| &d.id == id_ponte)
        .ok_or(format!("A ponte {id_ponte} não está na sua rede Tailscale."))?;
    if !ponte.online {
        return Err(format!("A ponte {} está offline no Tailscale.", ponte.nome));
    }
    let ip_ponte = ponte.ip.as_ref().ok_or("A ponte não tem IP no Tailscale.")?;

    acordar::enviar_pela_ponte(ip_ponte, config.porta_ssh, mac)
}

fn mostrar_janela(app: &AppHandle) {
    if let Some(janela) = app.get_webview_window("main") {
        let _ = janela.show();
        let _ = janela.unminimize();
        let _ = janela.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let abrir = MenuItem::with_id(app, "abrir", "Abrir", true, None::<&str>)?;
            let sair = MenuItem::with_id(app, "sair", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&abrir, &sair])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("WakeBridge")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, evento| match evento.id.as_ref() {
                    "abrir" => mostrar_janela(app),
                    "sair" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|icone, evento| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = evento
                    {
                        mostrar_janela(icone.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        // Fechar a janela só esconde; para sair de verdade, use o menu da bandeja
        .on_window_event(|janela, evento| {
            if let WindowEvent::CloseRequested { api, .. } = evento {
                api.prevent_close();
                let _ = janela.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            listar_dispositivos,
            ler_config,
            salvar_config,
            rdp_aberto,
            conectar_rdp,
            acordar
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}
