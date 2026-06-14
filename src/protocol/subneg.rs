use super::constants::*;

pub struct Subneg {
    send_subneg_cb: Option<Box<dyn Fn(u8, &[u8]) + Send + Sync>>,
    send_neg_cb: Option<Box<dyn Fn(u8, u8) + Send + Sync>>,
    auth_request_cb: Option<Box<dyn Fn(Vec<u8>) + Send + Sync>>,
    terminal_type: String,
    window_size_getter: Option<Box<dyn Fn() -> WindowSize + Send + Sync>>,
}

use crate::terminal::terminal::WindowSize;

impl Subneg {
    pub fn new() -> Self {
        Self {
            send_subneg_cb: None,
            send_neg_cb: None,
            auth_request_cb: None,
            terminal_type: "xterm-256color".to_string(),
            window_size_getter: None,
        }
    }

    pub fn set_send_subneg_callback<F: Fn(u8, &[u8]) + Send + Sync + 'static>(&mut self, cb: F) {
        self.send_subneg_cb = Some(Box::new(cb));
    }

    pub fn set_send_neg_callback<F: Fn(u8, u8) + Send + Sync + 'static>(&mut self, cb: F) {
        self.send_neg_cb = Some(Box::new(cb));
    }

    pub fn set_auth_request_callback<F: Fn(Vec<u8>) + Send + Sync + 'static>(&mut self, cb: F) {
        self.auth_request_cb = Some(Box::new(cb));
    }

    pub fn set_terminal_type(&mut self, ttype: &str) {
        self.terminal_type = ttype.to_string();
    }

    pub fn set_window_size_getter<F: Fn() -> WindowSize + Send + Sync + 'static>(&mut self, getter: F) {
        self.window_size_getter = Some(Box::new(getter));
    }

    pub fn handle(&mut self, option: u8, data: &[u8]) {
        match option {
            TELOPT_TTYPE => self.handle_ttype(data),
            TELOPT_NAWS => self.handle_naws(data),
            TELOPT_ECHO => {
                if !data.is_empty() && data[0] == WILL {
                    // WILL ECHO means server handles echo, disable local echo
                }
            }
            TELOPT_AUTHENTICATION => self.handle_auth(data),
            _ => {
                log::debug!("Unhandled subneg option: {}", option);
            }
        }
    }

    fn handle_ttype(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        if data[0] == TTYPE_SEND {
            // Server requests terminal type
            let mut response = Vec::with_capacity(1 + self.terminal_type.len());
            response.push(TTYPE_IS);
            response.extend_from_slice(self.terminal_type.as_bytes());
            if let Some(ref cb) = self.send_subneg_cb {
                cb(TELOPT_TTYPE, &response);
            }
            log::debug!("Sent TTYPE: {}", self.terminal_type);
        }
    }

    fn handle_naws(&self, data: &[u8]) {
        if data.len() >= 4 {
            let w = ((data[0] as u16) << 8) | data[1] as u16;
            let h = ((data[2] as u16) << 8) | data[3] as u16;
            log::debug!("Server NAWS: {}x{}", w, h);
        }
    }

    fn handle_auth(&self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let cmd = data[0];
        log::debug!("AUTH subneg command: {}", cmd);

        match cmd {
            AUTH_SEND => {
                if data.len() > 1 {
                    if let Some(ref cb) = self.auth_request_cb {
                        cb(data[1..].to_vec());
                    }
                }
            }
            AUTH_REPLY => {
                log::info!("AUTH REPLY received");
            }
            AUTH_NAME => {
                log::info!("AUTH NAME requested");
            }
            _ => {
                log::warn!("Unknown AUTH command: {}", cmd);
            }
        }
    }

    pub fn send_naws(&self) {
        if let (Some(ref getter), Some(ref cb)) = (&self.window_size_getter, &self.send_subneg_cb) {
            let ws = getter();
            let naws_data: [u8; 4] = [
                (ws.width >> 8) as u8,
                (ws.width & 0xFF) as u8,
                (ws.height >> 8) as u8,
                (ws.height & 0xFF) as u8,
            ];
            cb(TELOPT_NAWS, &naws_data);
        }
    }
}
