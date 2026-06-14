use tokio::sync::mpsc;

use crate::auth::prompt_detector::PromptDetector;
use crate::net::connection::Connection;
use crate::protocol::rlogin::{RloginConfig, RloginParser};
use crate::terminal::terminal::{DisplayMode, Terminal};

pub struct RloginSessionConfig {
    pub hostname: String,
    pub port: u16,
    pub timeout_sec: u32,
    pub client_user: String,
    pub server_user: String,
    pub password: String,
    pub terminal_type: String,
    pub terminal_speed: String,
    pub display_mode: DisplayMode,
    pub user_prompts: Vec<String>,
    pub passwd_prompts: Vec<String>,
}

pub enum RloginSessionEvent {
    ServerData(Vec<u8>),
    SendData(Vec<u8>),
    WindowResize,
    Close(String),
}

pub struct RloginSession {
    connection: Option<Connection>,
    parser: RloginParser,
    prompt_detector: PromptDetector,
    display_mode: DisplayMode,
    active: bool,
    event_tx: mpsc::UnboundedSender<RloginSessionEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<RloginSessionEvent>>,
}

impl RloginSession {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            connection: None,
            parser: RloginParser::new(),
            prompt_detector: PromptDetector::new(),
            display_mode: DisplayMode::Raw,
            active: false,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    pub fn event_sender(&self) -> mpsc::UnboundedSender<RloginSessionEvent> {
        self.event_tx.clone()
    }

    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<RloginSessionEvent>> {
        self.event_rx.take()
    }

    pub async fn start(&mut self, config: RloginSessionConfig) -> bool {
        self.display_mode = config.display_mode;

        self.prompt_detector.set_password(&config.password);
        self.prompt_detector.set_user_prompts(&config.user_prompts);
        self.prompt_detector.set_passwd_prompts(&config.passwd_prompts);

        // Create connection
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

        // Send rlogin handshake
        let rlogin_config = RloginConfig {
            client_user: config.client_user,
            server_user: config.server_user,
            terminal_type: config.terminal_type,
            terminal_speed: config.terminal_speed,
        };
        let handshake = self.parser.build_handshake(&rlogin_config);
        conn.send(&handshake).await;

        log::info!("Rlogin handshake sent to {}:{}", config.hostname, config.port);

        self.connection = Some(conn);

        // Spawn data forwarding task
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(data) = data_rx.recv() => {
                        let _ = event_tx.send(RloginSessionEvent::ServerData(data));
                    }
                    Some(err) = error_rx.recv() => {
                        log::error!("Rlogin connection error: {}", err);
                    }
                    Some(()) = close_rx.recv() => {
                        let _ = event_tx.send(RloginSessionEvent::Close("Connection closed".to_string()));
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
        log::info!("Rlogin session stopped: {}", reason);
    }

    pub fn send_input(&mut self, data: &[u8]) {
        if !self.active {
            return;
        }
        if let Some(conn) = self.connection.clone() {
            let data = data.to_vec();
            tokio::spawn(async move {
                conn.send(&data).await;
            });
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn notify_resize(&self) {
        if !self.active {
            return;
        }
        let _ = self.event_tx.send(RloginSessionEvent::WindowResize);
    }

    pub fn display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    pub fn handle_event(&mut self, event: RloginSessionEvent, terminal: &dyn Terminal) {
        match event {
            RloginSessionEvent::ServerData(data) => {
                self.parser.process(&data);
            }
            RloginSessionEvent::SendData(data) => {
        if let Some(conn) = self.connection.clone() {
            tokio::spawn(async move {
                conn.send(&data).await;
            });
        }
            }
            RloginSessionEvent::WindowResize => {
                let ws = terminal.get_window_size();
                let resize_data = RloginParser::build_window_resize(ws.height, ws.width);
                if let Some(conn) = self.connection.clone() {
                    tokio::spawn(async move {
                        conn.send(&resize_data).await;
                    });
                }
                log::debug!("Rlogin window resize sent: {}x{}", ws.width, ws.height);
            }
            RloginSessionEvent::Close(reason) => {
                self.stop(&reason);
            }
        }
    }
}
