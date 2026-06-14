use std::collections::HashMap;

use super::constants::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionState {
    No,
    Yes,
    WantNo,
    WantYes,
    WantNoOpposite,
    WantYesOpposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionSide {
    Local,
    Remote,
}

struct OptionPair {
    local: OptionState,
    remote: OptionState,
}

impl OptionPair {
    fn new() -> Self {
        Self { local: OptionState::No, remote: OptionState::No }
    }
}

pub struct Negotiation {
    options: HashMap<u8, OptionPair>,
    send_cb: Option<Box<dyn Fn(u8, u8) + Send + Sync>>,
}

impl Negotiation {
    pub fn new() -> Self {
        Self { options: HashMap::new(), send_cb: None }
    }

    pub fn set_send_callback<F: Fn(u8, u8) + Send + Sync + 'static>(&mut self, cb: F) {
        self.send_cb = Some(Box::new(cb));
    }

    fn emit(&self, cmd: u8, option: u8) {
        if let Some(ref cb) = self.send_cb {
            cb(cmd, option);
        }
    }

    fn get_or_create(&mut self, option: u8) -> &mut OptionPair {
        self.options.entry(option).or_insert_with(OptionPair::new)
    }

    fn get_pair(&self, option: u8) -> Option<&OptionPair> {
        self.options.get(&option)
    }

    pub fn request_enable(&mut self, option: u8, side: OptionSide) {
        let pair = self.get_or_create(option);
        let state = match side {
            OptionSide::Local => &mut pair.local,
            OptionSide::Remote => &mut pair.remote,
        };
        let cmd = match side {
            OptionSide::Local => WILL,
            OptionSide::Remote => DO,
        };

        match *state {
            OptionState::No => {
                *state = OptionState::WantYes;
                self.emit(cmd, option);
            }
            OptionState::Yes => {}
            OptionState::WantNo => {
                *state = OptionState::WantNoOpposite;
            }
            OptionState::WantYes => {}
            OptionState::WantNoOpposite => {}
            OptionState::WantYesOpposite => {
                *state = OptionState::WantYes;
            }
        }
    }

    pub fn request_disable(&mut self, option: u8, side: OptionSide) {
        let pair = self.get_or_create(option);
        let state = match side {
            OptionSide::Local => &mut pair.local,
            OptionSide::Remote => &mut pair.remote,
        };
        let cmd = match side {
            OptionSide::Local => WONT,
            OptionSide::Remote => DONT,
        };

        match *state {
            OptionState::No => {}
            OptionState::Yes => {
                *state = OptionState::WantNo;
                self.emit(cmd, option);
            }
            OptionState::WantNo => {}
            OptionState::WantYes => {
                *state = OptionState::WantYesOpposite;
            }
            OptionState::WantNoOpposite => {
                *state = OptionState::WantNo;
            }
            OptionState::WantYesOpposite => {}
        }
    }

    pub fn handle_will(&mut self, option: u8) {
        let pair = self.get_or_create(option);
        match pair.remote {
            OptionState::No => {
                // Server wants to enable, we refuse by default
                self.emit(DONT, option);
            }
            OptionState::Yes => {}
            OptionState::WantNo => {
                pair.remote = OptionState::No;
                log::warn!("DONT answered by WILL for option {}", option);
            }
            OptionState::WantNoOpposite => {
                pair.remote = OptionState::Yes;
            }
            OptionState::WantYes => {
                pair.remote = OptionState::Yes;
            }
            OptionState::WantYesOpposite => {
                pair.remote = OptionState::WantNo;
                self.emit(DONT, option);
            }
        }
    }

    pub fn handle_wont(&mut self, option: u8) {
        let pair = self.get_or_create(option);
        match pair.remote {
            OptionState::No => {}
            OptionState::Yes => {
                pair.remote = OptionState::No;
                self.emit(DONT, option);
            }
            OptionState::WantNo => {
                pair.remote = OptionState::No;
            }
            OptionState::WantNoOpposite => {
                pair.remote = OptionState::WantYes;
                self.emit(DO, option);
            }
            OptionState::WantYes => {
                pair.remote = OptionState::No;
            }
            OptionState::WantYesOpposite => {
                pair.remote = OptionState::No;
            }
        }
    }

    pub fn handle_do(&mut self, option: u8) {
        let pair = self.get_or_create(option);
        match pair.local {
            OptionState::No => {
                // Server requests we enable, we refuse by default
                self.emit(WONT, option);
            }
            OptionState::Yes => {}
            OptionState::WantNo => {
                pair.local = OptionState::No;
                log::warn!("WONT answered by DO for option {}", option);
            }
            OptionState::WantNoOpposite => {
                pair.local = OptionState::Yes;
            }
            OptionState::WantYes => {
                pair.local = OptionState::Yes;
            }
            OptionState::WantYesOpposite => {
                pair.local = OptionState::WantNo;
                self.emit(WONT, option);
            }
        }
    }

    pub fn handle_dont(&mut self, option: u8) {
        let pair = self.get_or_create(option);
        match pair.local {
            OptionState::No => {}
            OptionState::Yes => {
                pair.local = OptionState::No;
                self.emit(WONT, option);
            }
            OptionState::WantNo => {
                pair.local = OptionState::No;
            }
            OptionState::WantNoOpposite => {
                pair.local = OptionState::WantYes;
                self.emit(WILL, option);
            }
            OptionState::WantYes => {
                pair.local = OptionState::No;
            }
            OptionState::WantYesOpposite => {
                pair.local = OptionState::No;
            }
        }
    }

    pub fn get_state(&self, option: u8, side: OptionSide) -> OptionState {
        match self.get_pair(option) {
            Some(pair) => match side {
                OptionSide::Local => pair.local,
                OptionSide::Remote => pair.remote,
            },
            None => OptionState::No,
        }
    }

    pub fn is_enabled(&self, option: u8, side: OptionSide) -> bool {
        self.get_state(option, side) == OptionState::Yes
    }
}
