use std::path::Path;

use tokio::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) fn sidecar_command(program: &Path) -> Command {
    let command = Command::new(program);
    #[cfg(target_os = "windows")]
    let command = {
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}
