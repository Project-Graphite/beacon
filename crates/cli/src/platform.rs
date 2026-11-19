use std::io;
use std::sync::OnceLock;
use windows_sys::Win32::System::Console::{
    ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE,
    SetConsoleCtrlHandler, SetConsoleMode,
};

static INTERRUPT: OnceLock<fn() -> bool> = OnceLock::new();

pub fn on_console_interrupt(handler: fn() -> bool) -> io::Result<()> {
    INTERRUPT
        .set(handler)
        .map_err(|_| io::Error::other("an interrupt handler is already installed"))?;
    if unsafe { SetConsoleCtrlHandler(Some(interrupted), 1) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

unsafe extern "system" fn interrupted(_kind: u32) -> i32 {
    i32::from(INTERRUPT.get().is_some_and(|handler| handler()))
}

pub fn enable_terminal_redraw() {
    unsafe {
        let console = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(console, &mut mode) != 0 {
            SetConsoleMode(console, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}
