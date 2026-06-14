use std::io::Write;

use clap::Parser;

use acktel::cli::args::{Args, DisplayModeArg, Protocol};
use acktel::cli::config::Config;
use acktel::core::shutdown::ShutdownManager;
use acktel::terminal::terminal::{DisplayMode, write_with_mode};

use acktel::core::session::SessionEvent;
use acktel::core::rlogin_session::RloginSessionEvent;

fn main() {
    let args = Args::parse();

    if args.help {
        acktel::cli::args::print_usage();
        return;
    }

    if args.version {
        println!("acktel version {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let hostname = match &args.hostname {
        Some(h) => h.clone(),
        None => {
            eprintln!("Hostname is required");
            acktel::cli::args::print_usage();
            std::process::exit(1);
        }
    };

    let log_level = match args.log_level.as_str() {
        "error" => "error",
        "warn" => "warn",
        "info" => "info",
        "debug" => "debug",
        _ => "info",
    };
    env_logger::Builder::new()
        .filter_level(log_level.parse().unwrap_or(log::LevelFilter::Info))
        .init();

    let _config = args.config.as_ref()
        .and_then(|path| Config::load(path).ok())
        .or_else(Config::load_default);

    let display_mode = match args.display {
        DisplayModeArg::Ignore => DisplayMode::Ignore,
        DisplayModeArg::Hex => DisplayMode::Hex,
        DisplayModeArg::Placeholder => DisplayMode::Placeholder,
        DisplayModeArg::Raw => DisplayMode::Raw,
    };

    let port = args.get_port();
    let terminal_type = args.terminal.clone();
    let username = args.username.clone().unwrap_or_default();
    let password = args.password.clone().unwrap_or_default();
    let user_prompts = args.user_prompt.clone();
    let passwd_prompts = args.passwd_prompt.clone();
    let timeout = args.timeout;

    // Enable crossterm raw mode for proper key event detection
    crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");

    let mut shutdown = ShutdownManager::new();
    shutdown.install_signal_handlers();

    match args.protocol {
        Protocol::Telnet => {
            // For telnet: username from -u, fallback to -l (local_user)
            let telnet_user = if !username.is_empty() {
                username.clone()
            } else if !args.local_user.as_deref().unwrap_or("").is_empty() {
                args.local_user.clone().unwrap_or_default()
            } else {
                username.clone()
            };

            run_telnet(
                &hostname, port, timeout, &terminal_type, display_mode,
                &telnet_user, &password, &user_prompts, &passwd_prompts,
                &mut shutdown,
            );
        }
        Protocol::Rlogin => {
            let local_user = args.local_user.clone()
                .or_else(|| std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok())
                .unwrap_or_else(|| "user".to_string());

            run_rlogin(
                &hostname, port, timeout, &terminal_type, display_mode,
                &local_user, &username, &password, &user_prompts, &passwd_prompts,
                &mut shutdown,
            );
        }
    }

    // Restore terminal
    let _ = crossterm::terminal::disable_raw_mode();
    println!("\r\nDisconnected");
}

fn run_telnet(
    hostname: &str,
    port: u16,
    timeout: u32,
    terminal_type: &str,
    display_mode: DisplayMode,
    username: &str,
    password: &str,
    user_prompts: &[String],
    passwd_prompts: &[String],
    shutdown: &mut ShutdownManager,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut session = acktel::core::session::Session::new();

        let config = acktel::core::session::SessionConfig {
            hostname: hostname.to_string(),
            port,
            timeout_sec: timeout,
            terminal_type: terminal_type.to_string(),
            display_mode,
            enable_auth: !username.is_empty(),
            username: username.to_string(),
            password: password.to_string(),
            user_prompts: user_prompts.to_vec(),
            passwd_prompts: passwd_prompts.to_vec(),
        };

        let close_cb_sender = session.event_sender();
        shutdown.set_callback(move || {
            let _ = close_cb_sender.send(SessionEvent::Close(
                "User requested disconnect".to_string(),
            ));
        });

        if !session.start(config).await {
            eprintln!("Failed to connect to {}:{} (telnet)", hostname, port);
            return;
        }

        let event_sender = session.event_sender();
        let mut event_rx = session.take_event_receiver().unwrap();

        // Spawn stdin reader using crossterm
        let stdin_event_tx = event_sender.clone();
        std::thread::spawn(move || {
            read_stdin_crossterm(stdin_event_tx);
        });

        // Main event loop
        while let Some(event) = event_rx.recv().await {
            match &event {
                SessionEvent::DisplayData(data) => {
                    let mode = session.display_mode();
                    write_with_mode(&mut std::io::stdout(), data, mode);
                    std::io::stdout().flush().ok();
                }
                _ => {}
            }
            session.handle_event(event);
            if !session.is_active() {
                break;
            }
        }
    });
}

fn run_rlogin(
    hostname: &str,
    port: u16,
    timeout: u32,
    terminal_type: &str,
    display_mode: DisplayMode,
    local_user: &str,
    remote_user: &str,
    password: &str,
    user_prompts: &[String],
    passwd_prompts: &[String],
    shutdown: &mut ShutdownManager,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut session = acktel::core::rlogin_session::RloginSession::new();

        let server_user = if remote_user.is_empty() {
            local_user.to_string()
        } else {
            remote_user.to_string()
        };

        let config = acktel::core::rlogin_session::RloginSessionConfig {
            hostname: hostname.to_string(),
            port,
            timeout_sec: timeout,
            client_user: local_user.to_string(),
            server_user,
            password: password.to_string(),
            terminal_type: terminal_type.to_string(),
            terminal_speed: "9600".to_string(),
            display_mode,
            user_prompts: user_prompts.to_vec(),
            passwd_prompts: passwd_prompts.to_vec(),
        };

        let close_cb_sender = session.event_sender();
        shutdown.set_callback(move || {
            let _ = close_cb_sender.send(RloginSessionEvent::Close(
                "User requested disconnect".to_string(),
            ));
        });

        if !session.start(config).await {
            eprintln!("Failed to connect to {}:{} (rlogin)", hostname, port);
            return;
        }

        let event_sender = session.event_sender();
        let mut event_rx = session.take_event_receiver().unwrap();

        // Spawn stdin reader using crossterm
        let stdin_event_tx = event_sender.clone();
        std::thread::spawn(move || {
            read_stdin_crossterm_rlogin(stdin_event_tx);
        });

        // Main event loop
        while let Some(event) = event_rx.recv().await {
            match &event {
                RloginSessionEvent::ServerData(data) => {
                    let mode = session.display_mode();
                    write_with_mode(&mut std::io::stdout(), data, mode);
                    std::io::stdout().flush().ok();
                }
                _ => {}
            }
            session.handle_event(event);
            if !session.is_active() {
                break;
            }
        }
    });
}

fn read_stdin_crossterm(event_tx: tokio::sync::mpsc::UnboundedSender<SessionEvent>) {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use std::time::Duration;

    loop {
        if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
            continue;
        }

        match event::read() {
            Ok(Event::Key(key_event)) if key_event.kind == KeyEventKind::Press => {
                let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key_event.modifiers.contains(KeyModifiers::ALT);

                // Alt+. → disconnect
                if alt && key_event.code == KeyCode::Char('.') {
                    let _ = event_tx.send(SessionEvent::Close(
                        "User requested disconnect".to_string(),
                    ));
                    return;
                }

                // Window resize events are not handled via crossterm in this simple version
                // TODO: Add crossterm resize event handling

                if ctrl {
                    if let KeyCode::Char(c) = key_event.code {
                        let data = vec![(c as u8) & 0x1F];
                        let _ = event_tx.send(SessionEvent::SendData(data));
                        continue;
                    }
                }

                if alt {
                    if let KeyCode::Char(c) = key_event.code {
                        let data = vec![0x1b, c as u8];
                        let _ = event_tx.send(SessionEvent::SendData(data));
                        continue;
                    }
                }

                match key_event.code {
                    KeyCode::Char(c) => {
                        let data = vec![c as u8];
                        let _ = event_tx.send(SessionEvent::SendData(data));
                    }
                    KeyCode::Enter => {
                        let _ = event_tx.send(SessionEvent::SendData(vec![b'\r']));
                    }
                    KeyCode::Backspace => {
                        let _ = event_tx.send(SessionEvent::SendData(vec![0x7f]));
                    }
                    KeyCode::Tab => {
                        let _ = event_tx.send(SessionEvent::SendData(vec![b'\t']));
                    }
                    KeyCode::Esc => {
                        let _ = event_tx.send(SessionEvent::SendData(vec![0x1b]));
                    }
                    KeyCode::Up => {
                        let _ = event_tx.send(SessionEvent::SendData(b"\x1b[A".to_vec()));
                    }
                    KeyCode::Down => {
                        let _ = event_tx.send(SessionEvent::SendData(b"\x1b[B".to_vec()));
                    }
                    KeyCode::Right => {
                        let _ = event_tx.send(SessionEvent::SendData(b"\x1b[C".to_vec()));
                    }
                    KeyCode::Left => {
                        let _ = event_tx.send(SessionEvent::SendData(b"\x1b[D".to_vec()));
                    }
                    KeyCode::Home => {
                        let _ = event_tx.send(SessionEvent::SendData(b"\x1b[H".to_vec()));
                    }
                    KeyCode::End => {
                        let _ = event_tx.send(SessionEvent::SendData(b"\x1b[F".to_vec()));
                    }
                    KeyCode::Null => {
                        let _ = event_tx.send(SessionEvent::SendData(vec![0x00]));
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let _ = event_tx.send(SessionEvent::Close(
        "User requested disconnect".to_string(),
    ));
}

fn read_stdin_crossterm_rlogin(event_tx: tokio::sync::mpsc::UnboundedSender<RloginSessionEvent>) {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use std::time::Duration;

    loop {
        if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
            continue;
        }

        match event::read() {
            Ok(Event::Key(key_event)) if key_event.kind == KeyEventKind::Press => {
                let alt = key_event.modifiers.contains(KeyModifiers::ALT);

                if alt && key_event.code == KeyCode::Char('.') {
                    let _ = event_tx.send(RloginSessionEvent::Close(
                        "User requested disconnect".to_string(),
                    ));
                    return;
                }

                match key_event.code {
                    KeyCode::Char(c) => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![c as u8]));
                    }
                    KeyCode::Enter => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![b'\r']));
                    }
                    KeyCode::Backspace => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![0x7f]));
                    }
                    KeyCode::Tab => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![b'\t']));
                    }
                    KeyCode::Esc => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![0x1b]));
                    }
                    KeyCode::Up => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(b"\x1b[A".to_vec()));
                    }
                    KeyCode::Down => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(b"\x1b[B".to_vec()));
                    }
                    KeyCode::Right => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(b"\x1b[C".to_vec()));
                    }
                    KeyCode::Left => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(b"\x1b[D".to_vec()));
                    }
                    KeyCode::Home => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(b"\x1b[H".to_vec()));
                    }
                    KeyCode::End => {
                        let _ = event_tx.send(RloginSessionEvent::SendData(b"\x1b[F".to_vec()));
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let _ = event_tx.send(RloginSessionEvent::Close(
        "User requested disconnect".to_string(),
    ));
}
