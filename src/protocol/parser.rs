use super::constants::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    Data,
    Send,
    Iac,
    Will,
    Wont,
    Do,
    Dont,
    Subnegotiation,
    Error,
}

#[derive(Debug, Clone)]
pub struct ParseEvent {
    pub event_type: EventType,
    pub option: u8,
    pub data: Vec<u8>,
    pub error_message: String,
}

impl ParseEvent {
    pub fn data(data: Vec<u8>) -> Self {
        Self { event_type: EventType::Data, option: 0, data, error_message: String::new() }
    }
    pub fn send(data: Vec<u8>) -> Self {
        Self { event_type: EventType::Send, option: 0, data, error_message: String::new() }
    }
    pub fn will(opt: u8) -> Self {
        Self { event_type: EventType::Will, option: opt, data: vec![], error_message: String::new() }
    }
    pub fn wont(opt: u8) -> Self {
        Self { event_type: EventType::Wont, option: opt, data: vec![], error_message: String::new() }
    }
    pub fn do_cmd(opt: u8) -> Self {
        Self { event_type: EventType::Do, option: opt, data: vec![], error_message: String::new() }
    }
    pub fn dont(opt: u8) -> Self {
        Self { event_type: EventType::Dont, option: opt, data: vec![], error_message: String::new() }
    }
    pub fn subnegotiation(opt: u8, data: Vec<u8>) -> Self {
        Self { event_type: EventType::Subnegotiation, option: opt, data, error_message: String::new() }
    }
    pub fn error(msg: &str) -> Self {
        Self { event_type: EventType::Error, option: 0, data: vec![], error_message: msg.to_string() }
    }
    pub fn iac(cmd: u8) -> Self {
        Self { event_type: EventType::Iac, option: cmd, data: vec![], error_message: String::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Data,
    Iac,
    Will,
    Wont,
    Do,
    Dont,
    Sb,
    SbData,
    SbIac,
}

pub struct Parser {
    state: ParserState,
    current_option: u8,
    sb_option: u8,
    sb_data: Vec<u8>,
    send_buf: Vec<u8>,
    callback: Option<Box<dyn Fn(ParseEvent) + Send + Sync>>,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Data,
            current_option: 0,
            sb_option: 0,
            sb_data: Vec::new(),
            send_buf: Vec::new(),
            callback: None,
        }
    }

    pub fn set_callback<F: Fn(ParseEvent) + Send + Sync + 'static>(&mut self, callback: F) {
        self.callback = Some(Box::new(callback));
    }

    fn emit(&self, event: ParseEvent) {
        if let Some(ref cb) = self.callback {
            cb(event);
        }
    }

    pub fn process(&mut self, data: &[u8]) {
        for &byte in data {
            match self.state {
                ParserState::Data => {
                    if byte == IAC {
                        self.state = ParserState::Iac;
                    } else {
                        self.emit(ParseEvent::data(vec![byte]));
                    }
                }
                ParserState::Iac => {
                    match byte {
                        WILL => self.state = ParserState::Will,
                        WONT => self.state = ParserState::Wont,
                        DO => self.state = ParserState::Do,
                        DONT => self.state = ParserState::Dont,
                        SB => {
                            self.state = ParserState::Sb;
                            self.sb_option = 0;
                            self.sb_data.clear();
                        }
                        IAC => {
                            // Escaped IAC - literal 0xFF
                            self.emit(ParseEvent::data(vec![IAC]));
                            self.state = ParserState::Data;
                        }
                        GA | EL | EC | AYT | AO | IP | BRK | DM | NOP => {
                            self.emit(ParseEvent::iac(byte));
                            self.state = ParserState::Data;
                        }
                        _ => {
                            self.emit(ParseEvent::error(&format!("Unknown IAC command: {}", byte)));
                            self.state = ParserState::Data;
                        }
                    }
                }
                ParserState::Will => {
                    self.current_option = byte;
                    self.emit(ParseEvent::will(byte));
                    self.state = ParserState::Data;
                }
                ParserState::Wont => {
                    self.current_option = byte;
                    self.emit(ParseEvent::wont(byte));
                    self.state = ParserState::Data;
                }
                ParserState::Do => {
                    self.current_option = byte;
                    self.emit(ParseEvent::do_cmd(byte));
                    self.state = ParserState::Data;
                }
                ParserState::Dont => {
                    self.current_option = byte;
                    self.emit(ParseEvent::dont(byte));
                    self.state = ParserState::Data;
                }
                ParserState::Sb => {
                    self.sb_option = byte;
                    self.state = ParserState::SbData;
                }
                ParserState::SbData => {
                    if byte == IAC {
                        self.state = ParserState::SbIac;
                    } else {
                        self.sb_data.push(byte);
                    }
                }
                ParserState::SbIac => {
                    if byte == SE {
                        // End of subnegotiation
                        let opt = self.sb_option;
                        let data = std::mem::take(&mut self.sb_data);
                        self.emit(ParseEvent::subnegotiation(opt, data));
                        self.state = ParserState::Data;
                    } else if byte == IAC {
                        // Escaped IAC in subneg data
                        self.sb_data.push(IAC);
                        self.state = ParserState::SbData;
                    } else {
                        // Protocol error - unexpected byte after IAC in subneg
                        self.emit(ParseEvent::error(&format!(
                            "Unexpected byte {} after IAC in subnegotiation", byte
                        )));
                        self.state = ParserState::Data;
                    }
                }
            }
        }
    }

    pub fn send_data(&mut self, data: &[u8]) {
        let mut escaped = Vec::with_capacity(data.len());
        for &byte in data {
            if byte == IAC {
                escaped.push(IAC);
                escaped.push(IAC);
            } else {
                escaped.push(byte);
            }
        }
        self.emit(ParseEvent::send(escaped));
    }

    pub fn send_will(&mut self, option: u8) {
        self.send_buf.clear();
        self.send_buf.extend_from_slice(&[IAC, WILL, option]);
        self.emit(ParseEvent::send(self.send_buf.clone()));
    }

    pub fn send_wont(&mut self, option: u8) {
        self.send_buf.clear();
        self.send_buf.extend_from_slice(&[IAC, WONT, option]);
        self.emit(ParseEvent::send(self.send_buf.clone()));
    }

    pub fn send_do(&mut self, option: u8) {
        self.send_buf.clear();
        self.send_buf.extend_from_slice(&[IAC, DO, option]);
        self.emit(ParseEvent::send(self.send_buf.clone()));
    }

    pub fn send_dont(&mut self, option: u8) {
        self.send_buf.clear();
        self.send_buf.extend_from_slice(&[IAC, DONT, option]);
        self.emit(ParseEvent::send(self.send_buf.clone()));
    }

    pub fn send_subnegotiation(&mut self, option: u8, data: &[u8]) {
        self.send_buf.clear();
        self.send_buf.push(IAC);
        self.send_buf.push(SB);
        self.send_buf.push(option);
        for &byte in data {
            if byte == IAC {
                self.send_buf.push(IAC);
            }
            self.send_buf.push(byte);
        }
        self.send_buf.push(IAC);
        self.send_buf.push(SE);
        self.emit(ParseEvent::send(self.send_buf.clone()));
    }
}
