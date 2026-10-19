//! Starting yt-dlp without a console window popping up each time (the GUI has no console).

/// CREATE_NO_WINDOW: start a console program without showing a window.
#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

pub fn command(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(NO_WINDOW);
    }
    command
}

pub fn async_command(program: &str) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut command = tokio::process::Command::new(program);
    #[cfg(windows)]
    command.creation_flags(NO_WINDOW);
    command
}
