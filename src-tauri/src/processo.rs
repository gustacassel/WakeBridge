use std::process::Command;

/// Command para programas de console (tailscale, ssh) sem abrir janela preta.
pub fn sem_janela(programa: &str) -> Command {
    #[allow(unused_mut)]
    let mut comando = Command::new(programa);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        comando.creation_flags(CREATE_NO_WINDOW);
    }
    comando
}
