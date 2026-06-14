use tokio::sync::mpsc;

use crate::auth::prompt_detector::PromptDetector;
use crate::net::connection::Connection;
use crate::protocol::constants::*;
use crate::protocol::parser::{Parser, ParseEvent};
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
    ServerData(Vec<u8>),
    SendData(Vec<u8>),
    WindowResize,
    Close(String),
}

pub struct Session {
    connection: Option<Connection>,
    parser: Parser,
    subneg: Subneg,
    prompt_detector: PromptDetector,
    terminal_type: String,
    display_mode: DisplayMode,
    username: String,
    password: String,
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
            terminal_type: "xterm-256color".to_string(),
            display_mode: DisplayMode::Raw,
            username: String::new(),
            password: String::new(),
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
        self.username = config.username.clone();
        self.password = config.password.clone();
        self.terminal_type = config.terminal_type.clone();

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

        // Set up parser callbacks - parser processes raw bytes and emits events
        let event_tx = self.event_tx.clone();
        self.parser.set_callback(move |event: ParseEvent| {
            // For now, just forward the raw data
            if !event.data.is_empty() {
                let _ = event_tx.send(SessionEvent::ServerData(event.data));
            }
        });

        // Create connection with channel plumbing
        let (data_tx, mut data_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (error_tx, mut error_rx) = mpsc::unbounded_channel::<String>();
        let (close_tx, mut close_rx) = mpsc::unbounded_channel::<()>();

        let conn = Connection::new(data_tx, error_tx, close_tx);

        // Connect
        let result = conn.connect(&config.hostname, config.port, config.timeout_sec).await;
        if !result.success {
            log::error!("Failed to connect: {}", result.error_message);
            return false;
        }

        self.active = true;

        // Send initial negotiation
        self.parser.send_will(TELOPT_TTYPE);
        self.parser.send_will(TELOPT_NAWS);
        self.parser.send_do(TELOPT_SGA);
        self.parser.send_do(TELOPT_ECHO);

        // Send initial NAWS
        self.subneg.send_naws();

        // Start read task
        let read_conn = conn.clone();
        tokio::spawn(async move {
            read_conn.start_read().await;
        });

        self.connection = Some(conn);

        // Spawn data forwarding task
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(data) = data_rx.recv() => {
                        let _ = event_tx.send(SessionEvent::ServerData(data));
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
            SessionEvent::ServerData(data) => {
                // Display handled by caller
                // Auto-respond to prompts
                let response = self.prompt_detector.detect_and_respond(&data);
                if !response.is_empty() {
                    let _ = self.event_tx.send(SessionEvent::SendData(response));
                }
            }
            SessionEvent::SendData(data) => {
                let _event_tx = self.event_tx.clone();
                if let Some(ref conn) = self.connection {
                    let conn = conn.clone();
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
