use std::sync::atomic::{AtomicBool, Ordering};

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

pub struct ShutdownManager {
    callback: Option<Box<dyn Fn() + Send + Sync>>,
}

impl ShutdownManager {
    pub fn new() -> Self {
        Self { callback: None }
    }

    pub fn set_callback<F: Fn() + Send + Sync + 'static>(&mut self, cb: F) {
        self.callback = Some(Box::new(cb));
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
    }
}

#[cfg(unix)]
extern "C" fn signal_handler(_sig: i32) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}
