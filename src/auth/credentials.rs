#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    CommandLine,
    ConfigFile,
    Environment,
    Interactive,
}

impl Default for CredentialSource {
    fn default() -> Self {
        CredentialSource::Interactive
    }
}

#[derive(Debug, Clone, Default)]
pub struct Credentials {
    pub source: CredentialSource,
    pub username: String,
    pub password: String,
    pub rsa_private_key: Vec<u8>,
}

impl Credentials {
    pub fn has_username(&self) -> bool {
        !self.username.is_empty()
    }

    pub fn has_password(&self) -> bool {
        !self.password.is_empty()
    }

    pub fn has_rsa_key(&self) -> bool {
        !self.rsa_private_key.is_empty()
    }

    pub fn clear_sensitive(&mut self) {
        self.password.zeroize();
        self.rsa_private_key.zeroize();
    }
}

trait Zeroize {
    fn zeroize(&mut self);
}

impl Zeroize for String {
    fn zeroize(&mut self) {
        unsafe {
            for byte in self.as_bytes_mut() {
                *byte = 0;
            }
        }
        self.clear();
    }
}

impl Zeroize for Vec<u8> {
    fn zeroize(&mut self) {
        for byte in self.iter_mut() {
            *byte = 0;
        }
        self.clear();
    }
}

pub fn collect_credentials(
    _hostname: &str,
    cli_username: &str,
    cli_password: &str,
) -> Credentials {
    let mut creds = Credentials {
        source: CredentialSource::CommandLine,
        username: cli_username.to_string(),
        password: cli_password.to_string(),
        rsa_private_key: Vec::new(),
    };

    if creds.username.is_empty() {
        if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
            creds.username = user;
            creds.source = CredentialSource::Environment;
        }
    }

    creds
}
