use anyhow::Result;
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};

static SIGNAL: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicUsize = AtomicUsize::new(0);

pub fn interrupt() {
    record(2);
}

fn record(signal: usize) {
    let _ = SIGNAL.compare_exchange(0, signal, Ordering::SeqCst, Ordering::SeqCst);
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

thread_local! {
    static IGNORED: Cell<usize> = const { Cell::new(0) };
}

pub struct Cleanup {
    previous: usize,
}

impl Cleanup {
    pub fn new() -> Self {
        Self {
            previous: IGNORED.replace(GENERATION.load(Ordering::SeqCst)),
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        IGNORED.set(self.previous);
    }
}

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
        for signal in [libc::SIGINT, libc::SIGTERM] {
            // The handler only updates lock-free atomics.
            unsafe {
                signal_hook::low_level::register(signal, move || record(signal as usize))?;
            }
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
    let signal = SIGNAL.load(Ordering::SeqCst);
    (signal != 0).then_some(signal as i32)
}

pub fn check() -> Result<()> {
    if GENERATION.load(Ordering::SeqCst) > IGNORED.get()
        && let Some(signal) = signal()
    {
        anyhow::bail!("Interrupted by signal {signal}");
    }
    Ok(())
}
