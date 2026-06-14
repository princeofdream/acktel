use super::credentials::Credentials;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    None,
    Simple,
    Rsa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    NotStarted,
    InProgress,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub struct AuthResult {
    pub success: bool,
    pub method_used: AuthMethod,
    pub error_message: String,
}

pub struct AuthManager {
    credentials: Credentials,
    state: AuthState,
    retry_count: u8,
    max_retries: u8,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            credentials: Credentials::default(),
            state: AuthState::NotStarted,
            retry_count: 0,
            max_retries: 3,
        }
    }

    pub fn set_credentials(&mut self, creds: Credentials) {
        self.credentials = creds;
    }

    pub fn negotiate_method(&self, server_methods: &[AuthMethod]) -> AuthMethod {
        let priority = [
            AuthMethod::Rsa,
            AuthMethod::Simple,
        ];

        for method in &priority {
            if server_methods.contains(method) {
                log::info!("Selected auth method: {:?}", method);
                return *method;
            }
        }
        AuthMethod::None
    }

    pub fn authenticate(&mut self, method: AuthMethod) -> AuthResult {
        if self.retry_count >= self.max_retries {
            self.state = AuthState::Failed;
            return AuthResult {
                success: false,
                method_used: method,
                error_message: format!("Maximum retries ({}) exceeded", self.max_retries),
            };
        }

        self.state = AuthState::InProgress;
        self.retry_count += 1;

        let result = match method {
            AuthMethod::Simple => self.do_simple_auth(),
            AuthMethod::Rsa => self.do_rsa_auth(),
            _ => AuthResult {
                success: false,
                method_used: method,
                error_message: "Unsupported auth method".to_string(),
            },
        };

        self.state = if result.success {
            AuthState::Success
        } else {
            AuthState::Failed
        };

        result
    }

    fn do_simple_auth(&self) -> AuthResult {
        if self.credentials.username.is_empty() {
            return AuthResult {
                success: false,
                method_used: AuthMethod::Simple,
                error_message: "No username provided".to_string(),
            };
        }
        AuthResult {
            success: true,
            method_used: AuthMethod::Simple,
            error_message: String::new(),
        }
    }

    fn do_rsa_auth(&self) -> AuthResult {
        if self.credentials.rsa_private_key.is_empty() {
            return AuthResult {
                success: false,
                method_used: AuthMethod::Rsa,
                error_message: "No RSA private key provided".to_string(),
            };
        }
        AuthResult {
            success: true,
            method_used: AuthMethod::Rsa,
            error_message: String::new(),
        }
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn retry_count(&self) -> u8 {
        self.retry_count
    }

    pub fn reset_retry_count(&mut self) {
        self.retry_count = 0;
        self.state = AuthState::NotStarted;
    }

    pub fn state(&self) -> AuthState {
        self.state
    }

    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }
}
