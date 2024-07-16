use crate::{
    errors::NodeResult,
    utils::{
        crypto::{sha256hash, sign_bytes_recoverable},
        get_current_time_nanos,
    },
};

use base64::{prelude::BASE64_STANDARD, Engine};
use core::fmt;
use ecies::PublicKey;
use libsecp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct P2PMessage {
    pub payload: String,
    pub topic: String,
    pub version: String,
    #[serde(default)]
    pub timestamp: u128,
}

/// 65-byte signature as hex characters take up 130 characters.
/// The 65-byte signature is composed of 64-byte RSV signature and 1-byte recovery id.
///
/// When recovery is not required and only verification is being done, we omit the recovery id
/// and therefore use 128 characters: SIGNATURE_SIZE - 2.
const SIGNATURE_SIZE_HEX: usize = 130;

impl P2PMessage {
    /// Creates a new ephemeral Waku message with current timestamp, version 0.
    ///
    /// - `payload` is gives as bytes. It is base64 encoded internally.
    /// - `topic` is the name of the topic itself within the full content topic. The rest of the content topic
    /// is filled in automatically, e.g. `/dria/0/<topic>/proto`.
    pub fn new(payload: impl AsRef<[u8]>, topic: &str) -> Self {
        Self {
            payload: BASE64_STANDARD.encode(payload),
            topic: topic.to_string(),
            version: env::var("CARGO_PKG_VERSION").unwrap_or_default(),
            timestamp: get_current_time_nanos(),
        }
    }

    /// Creates a new Waku Message by signing the SHA256 of the payload, and prepending the signature.
    pub fn new_signed(
        payload: impl AsRef<[u8]> + Clone,
        topic: &str,
        signing_key: &SecretKey,
    ) -> Self {
        let signature_bytes = sign_bytes_recoverable(&sha256hash(payload.clone()), signing_key);

        let mut signed_payload = Vec::new();
        signed_payload.extend_from_slice(signature_bytes.as_ref());
        signed_payload.extend_from_slice(payload.as_ref());
        Self::new(signed_payload, topic)
    }

    /// Decodes the base64 payload into bytes.
    pub fn decode_payload(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64_STANDARD.decode(&self.payload)
    }

    /// Decodes and parses the payload into JSON.
    pub fn parse_payload<T: for<'a> Deserialize<'a>>(&self, signed: bool) -> NodeResult<T> {
        let payload = self.decode_payload()?;

        let body = if signed {
            // skips the 65 byte hex signature
            &payload[SIGNATURE_SIZE_HEX..]
        } else {
            &payload[..]
        };

        let parsed: T = serde_json::from_slice(body)?;
        Ok(parsed)
    }

    pub fn is_signed(&self, public_key: &PublicKey) -> NodeResult<bool> {
        // decode base64 payload
        let payload = self.decode_payload()?;

        // parse signature (64 bytes = 128 hex chars, although the full 65-byte RSV signature is given)
        let (signature, body) = (
            &payload[..SIGNATURE_SIZE_HEX - 2],
            &payload[SIGNATURE_SIZE_HEX..],
        );
        let signature = hex::decode(signature).expect("could not decode");
        let signature =
            libsecp256k1::Signature::parse_standard_slice(&signature).expect("could not parse");

        // verify signature
        let digest = libsecp256k1::Message::parse(&sha256hash(body));
        Ok(libsecp256k1::verify(&digest, &signature, public_key))
    }
}

impl fmt::Display for P2PMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let payload_decoded = self
            .decode_payload()
            .unwrap_or(self.payload.as_bytes().to_vec());

        let payload_str = String::from_utf8(payload_decoded).unwrap_or(self.payload.clone());
        write!(
            f,
            "Message {} at {}\n{}",
            self.topic, self.timestamp, payload_str
        )
    }
}

impl TryFrom<libp2p::gossipsub::Message> for P2PMessage {
    type Error = serde_json::Error;

    fn try_from(value: libp2p::gossipsub::Message) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libsecp256k1::SecretKey;
    use rand::thread_rng;
    use serde_json::json;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestStruct {
        hello: String,
    }

    impl Default for TestStruct {
        fn default() -> Self {
            TestStruct {
                hello: "world".to_string(),
            }
        }
    }

    const TOPIC: &str = "test-topic";

    #[test]
    fn test_display_message() {
        let message = P2PMessage::new(b"hello world", "test-topic");
        println!("{}", message);
    }

    #[test]
    fn test_unsigned_message() {
        // create payload & message
        let body = TestStruct::default();
        let payload = serde_json::to_vec(&json!(body)).expect("Should serialize");
        let message = P2PMessage::new(payload, TOPIC);

        // decode message
        let message_body = message.decode_payload().expect("Should decode");
        let body = serde_json::from_slice::<TestStruct>(&message_body).expect("Should deserialize");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, "test-topic");
        assert_eq!(
            message.version,
            env::var("CARGO_PKG_VERSION").unwrap_or_default()
        );
        assert!(message.timestamp > 0);

        let parsed_body = message.parse_payload(false).expect("Should decode");
        assert_eq!(body, parsed_body);
    }

    #[test]
    fn test_signed_message() {
        let mut rng = thread_rng();
        let sk = SecretKey::random(&mut rng);
        let pk = PublicKey::from_secret_key(&sk);

        // create payload & message with signature & body
        let body = TestStruct::default();
        let body_str = serde_json::to_string(&body).unwrap();
        let message = P2PMessage::new_signed(body_str, TOPIC, &sk);

        // decode message
        let message_body = message.decode_payload().expect("Should decode");
        let body =
            serde_json::from_slice::<TestStruct>(&message_body[130..]).expect("Should parse");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, "test-topic");
        assert_eq!(
            message.version,
            env::var("CARGO_PKG_VERSION").unwrap_or_default()
        );
        assert!(message.timestamp > 0);

        assert!(message.is_signed(&pk).expect("Should check signature"));

        let parsed_body = message.parse_payload(true).expect("Should decode");
        assert_eq!(body, parsed_body);
    }
}
