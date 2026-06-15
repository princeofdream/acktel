use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
static CTRL_C_CALLBACK: Mutex<Option<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);

pub struct ShutdownManager {
    callback: Option<Box<dyn Fn() + Send + Sync>>,
}

impl ShutdownManager {
    pub fn new() -> Self {
        Self { callback: None }
    }

    pub fn set_callback<F: Fn() + Send + Sync + 'static>(&mut self, cb: F) {
        let arc_cb: Arc<dyn Fn() + Send + Sync> = Arc::new(cb);
        #[cfg(target_os = "windows")]
        {
            *CTRL_C_CALLBACK.lock().unwrap() = Some(arc_cb.clone());
        }
        // Wrap Arc back into Box for the local field
        let boxed: Box<dyn Fn() + Send + Sync> = Box::new(move || arc_cb());
        self.callback = Some(boxed);
    }

    pub fn request_shutdown(&self) {
        if !SHUTDOWN_REQUESTED.swap(true, Ordering::SeqCst) {
            log::info!("Shutdown requested");
            if let Some(ref cb) = self.callback {
                cb();
            }
        }
    }

    pub fn is_shutdown_requested(&self) -> bool {
        SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
    }

    pub fn install_signal_handlers(&self) {
        #[cfg(unix)]
        {
            unsafe {
                use nix::sys::signal::{signal, SigHandler, Signal};
                let _ = signal(Signal::SIGINT, SigHandler::Handler(signal_handler));
                let _ = signal(Signal::SIGTERM, SigHandler::Handler(signal_handler));
                let _ = signal(Signal::SIGPIPE, SigHandler::SigIgn);
            }
        }
        #[cfg(target_os = "windows")]
        {
            unsafe {
                let _ = windows::Win32::System::Console::SetConsoleCtrlHandler(Some(ctrl_handler), true);
            }
        }
    }
}

#[cfg(unix)]
extern "C" fn signal_handler(_sig: i32) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Console::CTRL_C_EVENT;
    if ctrl_type == CTRL_C_EVENT {
        SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        if let Ok(guard) = CTRL_C_CALLBACK.lock() {
            if let Some(ref cb) = *guard {
                cb();
            }
        }
        return BOOL(1);
    }
    BOOL(0)
}
