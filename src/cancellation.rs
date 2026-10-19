use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

static KEYBOARD_INTERRUPT: AtomicBool = AtomicBool::new(false);

pub fn interrupt() {
    KEYBOARD_INTERRUPT.store(true, AtomicOrdering::Relaxed);
}

#[cfg(unix)]
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(unix)]
static SIGNAL: OnceLock<Arc<AtomicUsize>> = OnceLock::new();

#[cfg(windows)]
unsafe extern "system" fn console_control(event: u32) -> windows_sys::core::BOOL {
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};
    if matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT) {
        interrupt();
        1
    } else {
        0
    }
}

pub fn install() -> Result<()> {
    #[cfg(unix)]
    {
        let flag = SIGNAL.get_or_init(|| Arc::new(AtomicUsize::new(0)));
        for signal in [libc::SIGINT, libc::SIGTERM] {
            signal_hook::flag::register_usize(signal, Arc::clone(flag), signal as usize)?;
        }
    }
    #[cfg(windows)]
    {
        use anyhow::Context;
        use windows_sys::Win32::System::Console::{GetConsoleCP, SetConsoleCtrlHandler};
        // Console-less processes cannot receive console control events.
        // Clear inherited Ctrl+C suppression only after installing our handler.
        if unsafe { GetConsoleCP() } != 0
            && (unsafe { SetConsoleCtrlHandler(Some(console_control), 1) } == 0
                || unsafe { SetConsoleCtrlHandler(None, 0) } == 0)
        {
            return Err(std::io::Error::last_os_error())
                .context("Failed to install console cancellation handler");
        }
    }
    Ok(())
}

pub fn signal() -> Option<i32> {
    if KEYBOARD_INTERRUPT.load(AtomicOrdering::Relaxed) {
        return Some(2);
    }
    #[cfg(unix)]
    if let Some(flag) = SIGNAL.get() {
        let signal = flag.load(Ordering::Relaxed);
        if signal != 0 {
            return Some(signal as i32);
        }
    }
    None
}

pub fn check() -> Result<()> {
    if let Some(signal) = signal() {
        anyhow::bail!("Interrupted by signal {signal}");
    }
    Ok(())
}
