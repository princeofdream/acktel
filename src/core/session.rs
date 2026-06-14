use tokio::sync::mpsc;

use crate::auth::prompt_detector::PromptDetector;
use crate::net::connection::Connection;
use crate::protocol::constants::*;
use crate::protocol::parser::{Parser, ParseEvent, EventType};
use crate::protocol::subneg::Subneg;
use crate::terminal::terminal::DisplayMode;

pub struct SessionConfig {
    pub hostname: String,
    pub port: u16,
    pub timeout_sec: u32,
    pub terminal_type: String,
    pub display_mode: DisplayMode,
    pub enable_auth: bool,
    pub username: String,
    pub password: String,
    pub user_prompts: Vec<String>,
    pub passwd_prompts: Vec<String>,
}

pub enum SessionEvent {
    /// Parsed data ready to display (IAC bytes stripped)
    DisplayData(Vec<u8>),
    /// Raw bytes from connection - needs parsing
    RawData(Vec<u8>),
    /// Send raw bytes to server
    SendData(Vec<u8>),
    WindowResize,
    Close(String),
}

pub struct Session {
    connection: Option<Connection>,
    parser: Parser,
    subneg: Subneg,
    prompt_detector: PromptDetector,
    display_mode: DisplayMode,
    active: bool,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<SessionEvent>>,
}

impl Session {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            connection: None,
            parser: Parser::new(),
            subneg: Subneg::new(),
            prompt_detector: PromptDetector::new(),
            display_mode: DisplayMode::Raw,
            active: false,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    pub fn event_sender(&self) -> mpsc::UnboundedSender<SessionEvent> {
        self.event_tx.clone()
    }

    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<SessionEvent>> {
        self.event_rx.take()
    }

    pub async fn start(&mut self, config: SessionConfig) -> bool {
        self.display_mode = config.display_mode;

        self.prompt_detector.set_username(&config.username);
        self.prompt_detector.set_password(&config.password);
        self.prompt_detector.set_user_prompts(&config.user_prompts);
        self.prompt_detector.set_passwd_prompts(&config.passwd_prompts);

        self.subneg.set_terminal_type(&config.terminal_type);

        // Set up subneg callbacks
        let event_tx = self.event_tx.clone();
        self.subneg.set_send_subneg_callback(move |opt: u8, data: &[u8]| {
            let mut full = vec![IAC, SB, opt];
            for &b in data {
                if b == IAC { full.push(IAC); }
                full.push(b);
            }
            full.extend_from_slice(&[IAC, SE]);
            let _ = event_tx.send(SessionEvent::SendData(full));
        });

        let event_tx = self.event_tx.clone();
        self.subneg.set_send_neg_callback(move |cmd: u8, option: u8| {
            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, cmd, option]));
        });

        let event_tx = self.event_tx.clone();
        self.subneg.set_auth_request_callback(move |_types: Vec<u8>| {
            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WONT, TELOPT_AUTHENTICATION]));
        });

        // Set up parser callback - processes IAC sequences, emits clean data
        let event_tx = self.event_tx.clone();
        self.parser.set_callback(move |event: ParseEvent| {
            match event.event_type {
                EventType::Data => {
                    log::debug!("Parser Data: {} bytes", event.data.len());
                    if !event.data.is_empty() {
                        let _ = event_tx.send(SessionEvent::DisplayData(event.data));
                    }
                }
                EventType::Send => {
                    log::debug!("Parser Send: {} bytes", event.data.len());
                    if !event.data.is_empty() {
                        let _ = event_tx.send(SessionEvent::SendData(event.data));
                    }
                }
                EventType::Will => {
                    log::debug!("Server WILL {}", event.option);
                    // For supported options, respond DO; otherwise DONT
                    match event.option {
                        TELOPT_TTYPE | TELOPT_NAWS | TELOPT_SGA | TELOPT_ECHO | TELOPT_BINARY => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, DO, event.option]));
                        }
                        _ => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, DONT, event.option]));
                        }
                    }
                }
                EventType::Wont => {
                    log::debug!("Server WONT {}", event.option);
                    let _ = event_tx.send(SessionEvent::SendData(vec![IAC, DONT, event.option]));
                }
                EventType::Do => {
                    log::debug!("Server DO {}", event.option);
                    match event.option {
                        TELOPT_TTYPE => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WILL, TELOPT_TTYPE]));
                        }
                        TELOPT_NAWS => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WILL, TELOPT_NAWS]));
                            // Send initial NAWS 80x24
                            let naws = vec![IAC, SB, TELOPT_NAWS, 0, 80, 0, 24, IAC, SE];
                            let _ = event_tx.send(SessionEvent::SendData(naws));
                        }
                        TELOPT_SGA => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WILL, TELOPT_SGA]));
                        }
                        TELOPT_ECHO => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WILL, TELOPT_ECHO]));
                        }
                        TELOPT_BINARY => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WILL, TELOPT_BINARY]));
                        }
                        _ => {
                            let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WONT, event.option]));
                        }
                    }
                }
                EventType::Dont => {
                    log::debug!("Server DONT {}", event.option);
                    let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WONT, event.option]));
                }
                EventType::Subnegotiation => {
                    log::debug!("Subneg option={} len={}", event.option, event.data.len());
                    match event.option {
                        TELOPT_TTYPE => {
                            if !event.data.is_empty() && event.data[0] == TTYPE_SEND {
                                let ttype = "xterm-256color";
                                let mut resp = vec![IAC, SB, TELOPT_TTYPE, TTYPE_IS];
                                resp.extend_from_slice(ttype.as_bytes());
                                resp.extend_from_slice(&[IAC, SE]);
                                let _ = event_tx.send(SessionEvent::SendData(resp));
                            }
                        }
                        TELOPT_NAWS => {
                            if event.data.len() >= 4 {
                                let w = ((event.data[0] as u16) << 8) | event.data[1] as u16;
                                let h = ((event.data[2] as u16) << 8) | event.data[3] as u16;
                                log::info!("Server NAWS: {}x{}", w, h);
                            }
                        }
                        TELOPT_AUTHENTICATION => {
                            if !event.data.is_empty() && event.data[0] == AUTH_SEND {
                                let _ = event_tx.send(SessionEvent::SendData(vec![IAC, WONT, TELOPT_AUTHENTICATION]));
                            }
                        }
                        _ => {
                            log::debug!("Unhandled subneg option: {}", event.option);
                        }
                    }
                }
                EventType::Error => {
                    log::error!("Protocol error: {}", event.error_message);
                }
                _ => {}
            }
        });

        // Create connection
        let (data_tx, mut data_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (error_tx, mut error_rx) = mpsc::unbounded_channel::<String>();
        let (close_tx, mut close_rx) = mpsc::unbounded_channel::<()>();

        let conn = Connection::new(data_tx, error_tx, close_tx);

        let result = conn.connect(&config.hostname, config.port, config.timeout_sec).await;
        if !result.success {
            log::error!("Failed to connect: {}", result.error_message);
            return false;
        }

        self.active = true;

        // Send initial negotiation: WILL TTYPE, WILL NAWS, DO SGA, DO ECHO
        self.parser.send_will(TELOPT_TTYPE);
        self.parser.send_will(TELOPT_NAWS);
        self.parser.send_do(TELOPT_SGA);
        self.parser.send_do(TELOPT_ECHO);

        // Proactively send NAWS window size (like telcli does)
        // Some servers require this before sending the login prompt
        self.parser.send_subnegotiation(TELOPT_NAWS, &[0, 80, 0, 24]);

        // Start read task
        self.connection = Some(conn);

        // Spawn data forwarding task - routes raw bytes from connection to parser
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(data) = data_rx.recv() => {
                        let _ = event_tx.send(SessionEvent::RawData(data));
                    }
                    Some(err) = error_rx.recv() => {
                        log::error!("Connection error: {}", err);
                    }
                    Some(()) = close_rx.recv() => {
                        let _ = event_tx.send(SessionEvent::Close("Connection closed".to_string()));
                        break;
                    }
                    else => break,
                }
            }
        });

        true
    }

    pub fn stop(&mut self, reason: &str) {
        if !self.active {
            return;
        }
        self.active = false;
        self.connection = None;
        log::info!("Session stopped: {}", reason);
    }

    pub fn send_input(&mut self, data: &[u8]) {
        if !self.active {
            return;
        }
        self.parser.send_data(data);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn notify_resize(&self) {
        if !self.active {
            return;
        }
        self.subneg.send_naws();
        log::debug!("Session notify_resize: sent NAWS");
    }

    pub fn display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    pub fn handle_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::DisplayData(data) => {
                log::debug!("DisplayData: {} bytes", data.len());
                // Clean data from parser, display it
                let response = self.prompt_detector.detect_and_respond(&data);
                if !response.is_empty() {
                    let _ = self.event_tx.send(SessionEvent::SendData(response));
                }
            }
            SessionEvent::RawData(data) => {
                log::debug!("RawData: {} bytes, first={:02x}", data.len(), data.first().unwrap_or(&0));
                // Raw bytes from connection - process through parser
                self.parser.process(&data);
            }
            SessionEvent::SendData(data) => {
                log::debug!("SendData: {} bytes", data.len());
                if let Some(conn) = self.connection.clone() {
                    tokio::spawn(async move {
                        conn.send(&data).await;
                    });
                }
            }
            SessionEvent::WindowResize => {
                self.subneg.send_naws();
            }
            SessionEvent::Close(reason) => {
                self.stop(&reason);
            }
        }
    }
}
