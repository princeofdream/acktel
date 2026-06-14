use super::credentials::Credentials;
use crate::protocol::constants::*;

pub struct PlainAuth;

impl PlainAuth {
    pub fn build_auth_packet(creds: &Credentials) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.push(AUTH_IS);
        packet.push(AUTH_TYPE_NULL);
        packet.push(AUTH_MOD_CLIENT_TO_SERVER | AUTH_MOD_ONE_WAY);

        // Username (NUL terminated)
        packet.extend_from_slice(creds.username.as_bytes());
        packet.push(0);

        // Password (NUL terminated)
        packet.extend_from_slice(creds.password.as_bytes());
        packet.push(0);

        packet
    }

    pub fn handle_response(data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        match data[0] {
            AUTH_REPLY => {
                log::info!("Plain auth: server accepted");
                true
            }
            _ => {
                log::warn!("Plain auth: unexpected response {}", data[0]);
                false
            }
        }
    }
}
