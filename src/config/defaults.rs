use hex_literal::hex;

pub const DEFAULT_DKN_WAKU_URL: &str = "http://127.0.0.1:8645";

/// 32 byte secret key hex(b"node") * 8
pub const DEFAULT_DKN_WALLET_SECRET_KEY: &[u8; 32] =
    &hex!("6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465");

/// 33 byte compressed public key of secret key from hex(b"dria) * 8
/// TODO: update this to actual key of course
pub const DEFAULT_DKN_ADMIN_PUBLIC_KEY: &[u8; 33] =
    &hex!("0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658");
