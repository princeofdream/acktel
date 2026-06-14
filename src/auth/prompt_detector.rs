#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptType {
    None,
    Username,
    Password,
}

#[derive(Debug, Clone)]
pub struct PromptDetectResult {
    pub prompt_type: PromptType,
    pub matched_prompt: String,
}

pub struct PromptDetector {
    user_prompts: Vec<String>,
    passwd_prompts: Vec<String>,
    username: String,
    password: String,
    username_sent: bool,
    password_sent: bool,
}

impl PromptDetector {
    pub fn new() -> Self {
        let user_prompts = vec![
            "login:".to_string(),
            "username:".to_string(),
            "user:".to_string(),
            "account:".to_string(),
            "name:".to_string(),
            "login :".to_string(),
            "username :".to_string(),
            "user :".to_string(),
            "account :".to_string(),
            "name :".to_string(),
            "\u{8d26}\u{53f7}:".to_string(),
            "\u{7528}\u{6237}\u{540d}:".to_string(),
            "\u{7528}\u{6237}:".to_string(),
            "\u{4ee3}\u{53f7}:".to_string(),
            "\u{540d}\u{5b57}:".to_string(),
            "\u{8d26}\u{53f7}\u{ff1a}".to_string(),
            "\u{7528}\u{6237}\u{540d}\u{ff1a}".to_string(),
            "\u{7528}\u{6237}\u{ff1a}".to_string(),
            "\u{4ee3}\u{53f7}\u{ff1a}".to_string(),
            "\u{540d}\u{5b57}\u{ff1a}".to_string(),
            "\u{8bf7}\u{8f93}\u{5165}\u{8d26}\u{53f7}".to_string(),
            "\u{8bf7}\u{8f93}\u{5165}\u{7528}\u{6237}\u{540d}".to_string(),
            "\u{8bf7}\u{8f93}\u{5165}\u{4ee3}\u{53f7}".to_string(),
        ];

        let passwd_prompts = vec![
            "password:".to_string(),
            "passwd:".to_string(),
            "pass:".to_string(),
            "passcode:".to_string(),
            "passphrase:".to_string(),
            "password :".to_string(),
            "passwd :".to_string(),
            "pass :".to_string(),
            "passcode :".to_string(),
            "passphrase :".to_string(),
            "\u{5bc6}\u{7801}:".to_string(),
            "\u{53e3}\u{4ee4}:".to_string(),
            "\u{5bc6}\u{7801}\u{ff1a}".to_string(),
            "\u{53e3}\u{4ee4}\u{ff1a}".to_string(),
            "\u{8bf7}\u{8f93}\u{5165}\u{5bc6}\u{7801}".to_string(),
            "\u{8bf7}\u{8f93}\u{5165}\u{53e3}\u{4ee4}".to_string(),
        ];

        Self {
            user_prompts,
            passwd_prompts,
            username: String::new(),
            password: String::new(),
            username_sent: false,
            password_sent: false,
        }
    }

    pub fn set_user_prompts(&mut self, prompts: &[String]) {
        self.user_prompts.extend_from_slice(prompts);
    }

    pub fn set_passwd_prompts(&mut self, prompts: &[String]) {
        self.passwd_prompts.extend_from_slice(prompts);
    }

    pub fn set_username(&mut self, username: &str) {
        self.username = username.to_string();
        self.username_sent = false;
    }

    pub fn set_password(&mut self, password: &str) {
        self.password = password.to_string();
        self.password_sent = false;
    }

    pub fn detect_and_respond(&mut self, data: &[u8]) -> Vec<u8> {
        let result = self.detect(data);
        if result.prompt_type == PromptType::None {
            return Vec::new();
        }

        let mut response = String::new();

        if result.prompt_type == PromptType::Username && !self.username_sent && !self.username.is_empty() {
            response = format!("{}\r\n", self.username);
            self.username_sent = true;
        } else if result.prompt_type == PromptType::Password && !self.password_sent && !self.password.is_empty() {
            response = format!("{}\r\n", self.password);
            self.password_sent = true;
        }

        response.into_bytes()
    }

    pub fn detect(&self, data: &[u8]) -> PromptDetectResult {
        if data.is_empty() {
            return PromptDetectResult {
                prompt_type: PromptType::None,
                matched_prompt: String::new(),
            };
        }

        let text = String::from_utf8_lossy(data);
        let lower_text = Self::to_lower_ascii(&text);

        // Check username first if not sent
        if !self.username_sent && !self.username.is_empty() {
            if let Some(matched) = Self::contains_prompt(&lower_text, &self.user_prompts) {
                return PromptDetectResult {
                    prompt_type: PromptType::Username,
                    matched_prompt: matched,
                };
            }
        }

        // Check password
        if !self.password_sent && !self.password.is_empty() {
            if let Some(matched) = Self::contains_prompt(&lower_text, &self.passwd_prompts) {
                return PromptDetectResult {
                    prompt_type: PromptType::Password,
                    matched_prompt: matched,
                };
            }
        }

        // Even if sent, detect for external callers
        if let Some(matched) = Self::contains_prompt(&lower_text, &self.user_prompts) {
            return PromptDetectResult {
                prompt_type: PromptType::Username,
                matched_prompt: matched,
            };
        }
        if let Some(matched) = Self::contains_prompt(&lower_text, &self.passwd_prompts) {
            return PromptDetectResult {
                prompt_type: PromptType::Password,
                matched_prompt: matched,
            };
        }

        PromptDetectResult {
            prompt_type: PromptType::None,
            matched_prompt: String::new(),
        }
    }

    pub fn has_username(&self) -> bool {
        !self.username.is_empty() && !self.username_sent
    }

    pub fn has_password(&self) -> bool {
        !self.password.is_empty() && !self.password_sent
    }

    fn to_lower_ascii(input: &str) -> String {
        input
            .chars()
            .map(|c| {
                if c >= 'A' && c <= 'Z' {
                    (c as u8 + 0x20) as char
                } else {
                    c
                }
            })
            .collect()
    }

    fn contains_prompt(lower_text: &str, prompts: &[String]) -> Option<String> {
        for prompt in prompts {
            let lower_prompt = Self::to_lower_ascii(prompt);
            if lower_text.contains(&lower_prompt) {
                return Some(prompt.clone());
            }
        }
        None
    }
}
