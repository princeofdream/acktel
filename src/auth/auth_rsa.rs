use crate::protocol::constants::*;

pub struct RSAAuth;

impl RSAAuth {
    pub fn build_auth_packet(signed_data: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.push(AUTH_IS);
        packet.push(AUTH_TYPE_RSA);
        packet.push(AUTH_MOD_CLIENT_TO_SERVER | AUTH_MOD_ONE_WAY);
        packet.extend_from_slice(signed_data);
        packet
    }

    pub fn handle_response(data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        if data[0] == AUTH_REPLY {
            log::info!("RSA auth: server accepted");
            true
        } else {
            log::warn!("RSA auth: unexpected response");
            false
        }
    }
}
