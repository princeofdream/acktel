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

    // Enable raw mode for proper key event detection on non-Windows platforms
    // On Windows, read_stdin_platform uses Console API (ReadConsoleInputW) directly
    #[cfg(not(target_os = "windows"))]
    {
        crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");
    }

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
    #[cfg(not(target_os = "windows"))]
    {
        let _ = crossterm::terminal::disable_raw_mode();
    }
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

        // Spawn stdin reader using platform-specific API
        let stdin_event_tx = event_sender.clone();
        std::thread::spawn(move || {
            read_stdin_platform(stdin_event_tx);
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

/// Read stdin - cross-platform
fn read_stdin_platform(event_tx: tokio::sync::mpsc::UnboundedSender<SessionEvent>) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Console::*;

        unsafe {
            let h_stdin = GetStdHandle(STD_INPUT_HANDLE).unwrap_or_default();
            let mut escape_buf: Vec<u8> = Vec::new();

            log::debug!("stdin thread started (Windows Console API)");

            loop {
                let mut ir: [INPUT_RECORD; 128] = std::mem::zeroed();
                let mut count = 0u32;
                if ReadConsoleInputW(h_stdin, &mut ir, &mut count).is_err() {
                    log::error!("ReadConsoleInputW failed");
                    break;
                }

                for i in 0..count as usize {
                    let record = &ir[i];

                    match record.EventType {
                        // KEY_EVENT
                        1 => {
                            let ke = &record.Event.KeyEvent;
                            if !ke.bKeyDown.as_bool() { continue; }

                            let dw = ke.dwControlKeyState;
                            let ctrl = (dw & 0x0008) != 0 || (dw & 0x0004) != 0;
                            let alt = (dw & 0x0002) != 0 || (dw & 0x0001) != 0;
                            let ch = ke.uChar.UnicodeChar;
                            let vkey = ke.wVirtualKeyCode;

                            // Alt+. → disconnect
                            if alt && ch == '.' as u16 {
                                let _ = event_tx.send(SessionEvent::Close("disconnect".to_string()));
                                return;
                            }

                            // Ctrl+C → send 0x03 to server (don't exit)
                            if ctrl && (ch == 0x03 || vkey == 0x43) {
                                escape_buf.clear();
                                let _ = event_tx.send(SessionEvent::SendData(vec![0x03]));
                                continue;
                            }

                            // Ctrl+D → send 0x04 to server
                            if ctrl && (ch == 0x04 || vkey == 0x44) {
                                escape_buf.clear();
                                let _ = event_tx.send(SessionEvent::SendData(vec![0x04]));
                                continue;
                            }

                            // Ctrl+Z → send 0x1A to server
                            if ctrl && (ch == 0x1A || vkey == 0x5A) {
                                escape_buf.clear();
                                let _ = event_tx.send(SessionEvent::SendData(vec![0x1A]));
                                continue;
                            }

                            // Escape sequence handling (arrow keys, function keys, etc.)
                            if vkey == 0x1B {
                                escape_buf.clear();
                                escape_buf.push(0x1B);
                                continue;
                            }

                            // If we're building an escape sequence
                            if !escape_buf.is_empty() {
                                escape_buf.push(ch as u8);
                                // Common final bytes for escape sequences
                                let final_bytes = [
                                    'A', 'B', 'C', 'D', 'H', 'F', 'Z',
                                    '~', 'P', 'Q', 'R', 'S',
                                ];
                                if ch > 0 && final_bytes.contains(&(ch as u8 as char)) {
                                    let seq = escape_buf.clone();
                                    escape_buf.clear();
                                    let _ = event_tx.send(SessionEvent::SendData(seq));
                                }
                                continue;
                            }

                            // Enter
                            if vkey == 0x0D || ch == b'\r' as u16 {
                                let _ = event_tx.send(SessionEvent::SendData(vec![b'\r']));
                            }
                            // Backspace
                            else if vkey == 0x08 {
                                let _ = event_tx.send(SessionEvent::SendData(vec![0x7f]));
                            }
                            // Tab
                            else if vkey == 0x09 {
                                let _ = event_tx.send(SessionEvent::SendData(vec![b'\t']));
                            }
                            // Other control characters (Ctrl+A through Ctrl+Z)
                            else if ctrl && ch >= 1 && ch <= 26 {
                                let _ = event_tx.send(SessionEvent::SendData(vec![ch as u8]));
                            }
                            // Printable characters
                            else if ch > 0 && ch < 0x7F {
                                let _ = event_tx.send(SessionEvent::SendData(vec![ch as u8]));
                            }
                            // Unicode characters (CJK, emoji, etc.)
                            else if ch >= 0x7F {
                                let s = String::from_utf16_lossy(&[ch]);
                                let _ = event_tx.send(SessionEvent::SendData(s.into_bytes()));
                            }
                        }
                        // WINDOW_BUFFER_SIZE_EVENT → ignore
                        4 => {}
                        // Other events → ignore
                        _ => {}
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::io::Read;
        log::debug!("stdin thread started (Unix)");
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();
        let mut buf = [0u8; 256];

        loop {
            match handle.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    if data.len() >= 2 && data[0] == b'~' && data[1] == b'.' {
                        let _ = event_tx.send(SessionEvent::Close("disconnect".to_string()));
                        return;
                    }
                    let _ = event_tx.send(SessionEvent::SendData(data));
                }
                Err(e) => {
                    log::error!("stdin read error: {}", e);
                    break;
                }
            }
        }
    }

    let _ = event_tx.send(SessionEvent::Close("stdin ended".to_string()));
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
            read_stdin_rlogin(stdin_event_tx);
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

fn read_stdin_rlogin(event_tx: tokio::sync::mpsc::UnboundedSender<RloginSessionEvent>) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Console::*;

        unsafe {
            let h_stdin = GetStdHandle(STD_INPUT_HANDLE).unwrap_or_default();
            loop {
                let mut ir: [INPUT_RECORD; 128] = std::mem::zeroed();
                let mut count = 0u32;
                if ReadConsoleInputW(h_stdin, &mut ir, &mut count).is_err() {
                    break;
                }
                for i in 0..count as usize {
                    let record = &ir[i];
                    if record.EventType != 1 { continue; }
                    let ke = &record.Event.KeyEvent;
                    if !ke.bKeyDown.as_bool() { continue; }
                    let dw = ke.dwControlKeyState;
                    let alt = (dw & 0x0002) != 0 || (dw & 0x0001) != 0;
                    let ctrl = (dw & 0x0008) != 0 || (dw & 0x0004) != 0;
                    let ch = ke.uChar.UnicodeChar;
                    let vkey = ke.wVirtualKeyCode;

                    if alt && ch == '.' as u16 {
                        let _ = event_tx.send(RloginSessionEvent::Close("disconnect".to_string()));
                        return;
                    }
                    if vkey == 0x0D || ch == b'\r' as u16 {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![b'\r']));
                    } else if vkey == 0x08 {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![0x7f]));
                    } else if vkey == 0x09 {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![b'\t']));
                    } else if vkey == 0x1B {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![0x1b]));
                    } else if ctrl && ch >= 1 && ch <= 26 {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![ch as u8]));
                    } else if ch > 0 && ch < 0x7F {
                        let _ = event_tx.send(RloginSessionEvent::SendData(vec![ch as u8]));
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::io::Read;
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();
        let mut buf = [0u8; 256];
        loop {
            match handle.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    if data.len() >= 2 && data[0] == b'~' && data[1] == b'.' {
                        let _ = event_tx.send(RloginSessionEvent::Close("disconnect".to_string()));
                        return;
                    }
                    let _ = event_tx.send(RloginSessionEvent::SendData(data));
                }
                Err(_) => break,
            }
        }
    }
    let _ = event_tx.send(RloginSessionEvent::Close("stdin ended".to_string()));
}
