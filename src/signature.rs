use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Signs "timestamp.body" with the endpoint's secret.
/// Returns the signature as hex text, e.g. "a3f91c...".
pub fn sign(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");

    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);

    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_gives_same_signature() {
        let a = sign("0123456789abcdef", 1700000000, b"{\"x\":1}");
        let b = sign("0123456789abcdef", 1700000000, b"{\"x\":1}");
        assert_eq!(a, b);
    }

    #[test]
    fn different_secret_gives_different_signature() {
        let a = sign("0123456789abcdef", 1700000000, b"{\"x\":1}");
        let b = sign("fedcba9876543210", 1700000000, b"{\"x\":1}");
        assert_ne!(a, b);
    }

    #[test]
    fn changed_body_gives_different_signature() {
        let a = sign("0123456789abcdef", 1700000000, b"{\"x\":1}");
        let b = sign("0123456789abcdef", 1700000000, b"{\"x\":2}");
        assert_ne!(a, b);
    }
}