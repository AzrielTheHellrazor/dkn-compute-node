pub mod defaults;

use hex;
use libsecp256k1::SecretKey;
use std::env;

use self::defaults::{
    DEFAULT_DKN_HEARTBEAT_TIMEOUT, DEFAULT_DKN_OLLAMA_HOST, DEFAULT_DKN_OLLAMA_PORT,
    DEFAULT_DKN_WAKU_URL, DEFAULT_DKN_WALLET_PRIVKEY,
};

pub struct ComputeNodeConfig {
    /// Waku container URL
    pub DKN_WAKU_URL: String,
    /// Wallet Private Key as hexadecimal string, used by Waku as well.
    pub DKN_WALLET_PRIVKEY: SecretKey,
    /// Milliseconds of timeout between each heartbeat message check.
    pub DKN_HEARTBEAT_TIMEOUT: u16,
    /// Ollama container host
    pub DKN_OLLAMA_HOST: String,
    /// Ollama container port
    pub DKN_OLLAMA_PORT: u16,
}

impl Default for ComputeNodeConfig {
    fn default() -> Self {
        Self {
            DKN_WAKU_URL: env::var("DKN_WAKU_URL").unwrap_or(DEFAULT_DKN_WAKU_URL.to_string()),

            DKN_WALLET_PRIVKEY: SecretKey::parse_slice(
                hex::decode(
                    env::var("DKN_WALLET_PRIVKEY")
                        .unwrap_or(DEFAULT_DKN_WALLET_PRIVKEY.to_string()),
                )
                .unwrap()
                .as_slice(),
            )
            .expect("Could not parse key."),

            DKN_HEARTBEAT_TIMEOUT: env::var("DKN_HEARTBEAT_TIMEOUT")
                .unwrap_or(DEFAULT_DKN_HEARTBEAT_TIMEOUT.to_string())
                .parse::<u16>()
                .expect("Could not parse heartbeat timeout."),

            DKN_OLLAMA_HOST: env::var("DKN_OLLAMA_HOST")
                .unwrap_or(DEFAULT_DKN_OLLAMA_HOST.to_string()),

            DKN_OLLAMA_PORT: env::var("DKN_OLLAMA_PORT")
                .unwrap_or(DEFAULT_DKN_OLLAMA_PORT.to_string())
                .parse::<u16>()
                .expect("Could not parse port."),
        }
    }
}
