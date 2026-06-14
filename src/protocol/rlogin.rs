use super::constants::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RloginState {
    NotConnected,
    HandshakeSent,
    Connected,
    Failed,
}

#[derive(Debug, Clone)]
pub struct RloginConfig {
    pub client_user: String,
    pub server_user: String,
    pub terminal_type: String,
    pub terminal_speed: String,
}

pub struct RloginParser {
    state: RloginState,
    data_cb: Option<Box<dyn Fn(Vec<u8>) + Send + Sync>>,
    error_cb: Option<Box<dyn Fn(String) + Send + Sync>>,
}

impl RloginParser {
    pub fn new() -> Self {
        Self {
            state: RloginState::NotConnected,
            data_cb: None,
            error_cb: None,
        }
    }

    pub fn set_data_callback<F: Fn(Vec<u8>) + Send + Sync + 'static>(&mut self, cb: F) {
        self.data_cb = Some(Box::new(cb));
    }

    pub fn set_error_callback<F: Fn(String) + Send + Sync + 'static>(&mut self, cb: F) {
        self.error_cb = Some(Box::new(cb));
    }

    pub fn build_handshake(&mut self, config: &RloginConfig) -> Vec<u8> {
        let mut packet = Vec::new();

        // NUL byte
        packet.push(RLOGIN_NUL);

        // Client user
        packet.extend_from_slice(config.client_user.as_bytes());
        packet.push(RLOGIN_NUL);

        // Server user
        let server_user = if config.server_user.is_empty() {
            &config.client_user
        } else {
            &config.server_user
        };
        packet.extend_from_slice(server_user.as_bytes());
        packet.push(RLOGIN_NUL);

        // Terminal type/speed
        let mut term_speed = config.terminal_type.clone();
        if !config.terminal_speed.is_empty() {
            term_speed.push('/');
            term_speed.push_str(&config.terminal_speed);
        }
        packet.extend_from_slice(term_speed.as_bytes());
        packet.push(RLOGIN_NUL);

        self.state = RloginState::HandshakeSent;

        log::info!(
            "Rlogin handshake built: client={} server={} term={}",
            config.client_user,
            server_user,
            term_speed
        );

        packet
    }

    pub fn process(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        match self.state {
            RloginState::HandshakeSent => {
                if data[0] == RLOGIN_NUL {
                    self.state = RloginState::Connected;
                    log::info!("Rlogin handshake confirmed by server");

                    if data.len() > 1 {
                        if let Some(ref cb) = self.data_cb {
                            cb(data[1..].to_vec());
                        }
                    }
                } else {
                    self.state = RloginState::Failed;
                    let msg = String::from_utf8_lossy(data).to_string();
                    log::error!("Rlogin handshake rejected: {}", msg);
                    if let Some(ref cb) = self.error_cb {
                        cb(format!("Rlogin handshake rejected: {}", msg));
                    }
                }
            }
            RloginState::Connected => {
                // Pure data stream mode
                // In rlogin, 0x01开头的是窗口大小通知，跳过
                let mut i = 0;
                while i < data.len() {
                    if data[i] == 0x01 && i + 4 < data.len() {
                        i += 5;
                    } else {
                        break;
                    }
                }

                if i < data.len() {
                    if let Some(ref cb) = self.data_cb {
                        cb(data[i..].to_vec());
                    }
                }
            }
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.state = RloginState::NotConnected;
    }

    pub fn state(&self) -> RloginState {
        self.state
    }

    pub fn build_window_resize(rows: u16, cols: u16) -> Vec<u8> {
        vec![
            0x01,
            ((rows >> 8) & 0xFF) as u8,
            (rows & 0xFF) as u8,
            ((cols >> 8) & 0xFF) as u8,
            (cols & 0xFF) as u8,
        ]
    }
}
