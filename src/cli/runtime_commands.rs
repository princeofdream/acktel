use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCommand {
    None,
    Disconnect,
    ToggleDisplay,
    Status,
    Renegotiate,
    Help,
}

pub struct RuntimeCommandParser {
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    Tilde,
}

impl RuntimeCommandParser {
    pub fn new() -> Self {
        Self { state: State::Normal }
    }

    pub fn process(&mut self, byte: u8) -> (RuntimeCommand, Vec<u8>) {
        match self.state {
            State::Normal => {
                if byte == b'~' {
                    self.state = State::Tilde;
                    (RuntimeCommand::None, Vec::new())
                } else {
                    (RuntimeCommand::None, vec![byte])
                }
            }
            State::Tilde => {
                self.state = State::Normal;
                match byte {
                    b'.' => (RuntimeCommand::Disconnect, Vec::new()),
                    b'd' => (RuntimeCommand::ToggleDisplay, Vec::new()),
                    b's' => (RuntimeCommand::Status, Vec::new()),
                    b'r' => (RuntimeCommand::Renegotiate, Vec::new()),
                    b'?' => {
                        Self::print_help();
                        (RuntimeCommand::Help, Vec::new())
                    }
                    b'~' => (RuntimeCommand::None, vec![b'~']),
                    _ => (RuntimeCommand::None, vec![b'~', byte]),
                }
            }
        }
    }

    pub fn print_help() {
        let _ = writeln!(
            std::io::stdout(),
            "\r\nRuntime commands (prefix with ~):
  ~.   Disconnect
  ~d   Toggle display mode
  ~s   Show connection status
  ~r   Renegotiate terminal options
  ~?   Show this help
  ~~   Send literal ~"
        );
    }
}
