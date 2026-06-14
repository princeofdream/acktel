use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u16,
    pub height: u16,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self { width: 80, height: 24 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Ignore,
    Hex,
    Placeholder,
    Raw,
}

pub trait Terminal: Send + Sync {
    fn set_raw_mode(&mut self, enable: bool) -> bool;
    fn set_echo_mode(&mut self, enable: bool) -> bool;
    fn get_window_size(&self) -> WindowSize;
    fn write(&self, data: &[u8]);
    fn get_terminal_type(&self) -> &str;
    fn supports_utf8(&self) -> bool;
}

pub fn write_with_mode(writer: &mut dyn Write, data: &[u8], mode: DisplayMode) {
    for &byte in data {
        match mode {
            DisplayMode::Raw => {
                let _ = writer.write_all(&[byte]);
            }
            DisplayMode::Hex => {
                if byte >= 0x20 && byte < 0x7F {
                    let _ = writer.write_all(&[byte]);
                } else {
                    let _ = write!(writer, "\\x{:02x}", byte);
                }
            }
            DisplayMode::Placeholder => {
                if byte >= 0x20 && byte < 0x7F {
                    let _ = writer.write_all(&[byte]);
                } else {
                    let _ = writer.write_all(b"?");
                }
            }
            DisplayMode::Ignore => {
                if byte >= 0x20 && byte < 0x7F {
                    let _ = writer.write_all(&[byte]);
                } else if byte == b'\r' || byte == b'\n' || byte == b'\t' || byte == b'\x1b' {
                    let _ = writer.write_all(&[byte]);
                }
            }
        }
    }
}

pub fn create_terminal() -> Box<dyn Terminal> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsTerminal::new())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(UnixTerminal::new())
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Console::*;

    pub struct WindowsTerminal {
        h_stdin: HANDLE,
        h_stdout: HANDLE,
        saved_in_mode: CONSOLE_MODE,
        saved_out_mode: CONSOLE_MODE,
        raw_mode: bool,
    }

    unsafe impl Send for WindowsTerminal {}
    unsafe impl Sync for WindowsTerminal {}

    impl WindowsTerminal {
        pub fn new() -> Self {
            unsafe {
                let h_stdin = GetStdHandle(STD_INPUT_HANDLE).unwrap_or(HANDLE::default());
                let h_stdout = GetStdHandle(STD_OUTPUT_HANDLE).unwrap_or(HANDLE::default());

                let mut saved_in_mode = CONSOLE_MODE(0);
                let mut saved_out_mode = CONSOLE_MODE(0);
                let _ = GetConsoleMode(h_stdin, &mut saved_in_mode);
                let _ = GetConsoleMode(h_stdout, &mut saved_out_mode);

                let _ = SetConsoleOutputCP(65001);
                let _ = SetConsoleCP(65001);

                Self {
                    h_stdin,
                    h_stdout,
                    saved_in_mode,
                    saved_out_mode,
                    raw_mode: false,
                }
            }
        }
    }

    impl Terminal for WindowsTerminal {
        fn set_raw_mode(&mut self, enable: bool) -> bool {
            if enable == self.raw_mode {
                return true;
            }

            unsafe {
                if enable {
                    let mut in_mode = self.saved_in_mode;
                    in_mode.0 &= !(ENABLE_LINE_INPUT.0 | ENABLE_ECHO_INPUT.0 | ENABLE_PROCESSED_INPUT.0);
                    in_mode.0 |= ENABLE_WINDOW_INPUT.0 | ENABLE_VIRTUAL_TERMINAL_INPUT.0;
                    if SetConsoleMode(self.h_stdin, in_mode).is_err() {
                        return false;
                    }

                    let mut out_mode = self.saved_out_mode;
                    out_mode.0 |= ENABLE_VIRTUAL_TERMINAL_PROCESSING.0 | DISABLE_NEWLINE_AUTO_RETURN.0;
                    let _ = SetConsoleMode(self.h_stdout, out_mode);

                    self.raw_mode = true;
                } else {
                    let _ = SetConsoleMode(self.h_stdin, self.saved_in_mode);
                    let _ = SetConsoleMode(self.h_stdout, self.saved_out_mode);
                    self.raw_mode = false;
                }
            }
            true
        }

        fn set_echo_mode(&mut self, enable: bool) -> bool {
            unsafe {
                let mut mode = CONSOLE_MODE(0);
                if GetConsoleMode(self.h_stdin, &mut mode).is_err() {
                    return false;
                }
                if enable {
                    mode.0 |= ENABLE_ECHO_INPUT.0;
                } else {
                    mode.0 &= !ENABLE_ECHO_INPUT.0;
                }
                SetConsoleMode(self.h_stdin, mode).is_ok()
            }
        }

        fn get_window_size(&self) -> WindowSize {
            unsafe {
                let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
                if GetConsoleScreenBufferInfo(self.h_stdout, &mut csbi).is_ok() {
                    let w = (csbi.srWindow.Right - csbi.srWindow.Left + 1) as u16;
                    let h = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1) as u16;
                    if w > 0 && h > 0 {
                        return WindowSize { width: w, height: h };
                    }
                }
                WindowSize::default()
            }
        }

        fn write(&self, data: &[u8]) {
            // Use stdio for output - simpler and doesn't require extra Windows features
            let _ = std::io::stdout().write_all(data);
            let _ = std::io::stdout().flush();
        }

        fn get_terminal_type(&self) -> &str {
            "xterm-256color"
        }

        fn supports_utf8(&self) -> bool {
            unsafe { GetConsoleOutputCP() == 65001 }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;
    use std::os::fd::AsRawFd;

    pub struct UnixTerminal {
        orig_termios: Option<nix::sys::termios::Termios>,
    }

    unsafe impl Send for UnixTerminal {}
    unsafe impl Sync for UnixTerminal {}

    impl UnixTerminal {
        pub fn new() -> Self {
            Self { orig_termios: None }
        }
    }

    impl Terminal for UnixTerminal {
        fn set_raw_mode(&mut self, enable: bool) -> bool {
            unsafe {
                let fd = std::io::stdin().as_raw_fd();
                if enable {
                    match nix::sys::termios::tcgetattr(fd) {
                        Ok(mut term) => {
                            self.orig_termios = Some(term.clone());
                            term.local_flags &= !(nix::sys::termios::LocalFlags::ECHO
                                | nix::sys::termios::LocalFlags::ICANON
                                | nix::sys::termios::LocalFlags::ISIG);
                            nix::sys::termios::tcsetattr(fd, nix::sys::termios::SetArg::TCSANOW, &term).is_ok()
                        }
                        Err(_) => false,
                    }
                } else {
                    if let Some(ref orig) = self.orig_termios {
                        nix::sys::termios::tcsetattr(fd, nix::sys::termios::SetArg::TCSANOW, orig).is_ok()
                    } else {
                        true
                    }
                }
            }
        }

        fn set_echo_mode(&mut self, enable: bool) -> bool {
            unsafe {
                let fd = std::io::stdin().as_raw_fd();
                if let Ok(mut term) = nix::sys::termios::tcgetattr(fd) {
                    if enable {
                        term.local_flags |= nix::sys::termios::LocalFlags::ECHO;
                    } else {
                        term.local_flags &= !nix::sys::termios::LocalFlags::ECHO;
                    }
                    nix::sys::termios::tcsetattr(fd, nix::sys::termios::SetArg::TCSANOW, &term).is_ok()
                } else {
                    false
                }
            }
        }

        fn get_window_size(&self) -> WindowSize {
            unsafe {
                let mut ws: libc::winsize = std::mem::zeroed();
                if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 {
                    if ws.ws_col > 0 && ws.ws_row > 0 {
                        return WindowSize { width: ws.ws_col, height: ws.ws_row };
                    }
                }
                WindowSize::default()
            }
        }

        fn write(&self, data: &[u8]) {
            let _ = std::io::stdout().write_all(data);
            let _ = std::io::stdout().flush();
        }

        fn get_terminal_type(&self) -> &str {
            "xterm-256color"
        }

        fn supports_utf8(&self) -> bool {
            true
        }
    }
}

#[cfg(target_os = "windows")]
use platform::WindowsTerminal;

#[cfg(not(target_os = "windows"))]
use platform::UnixTerminal;
