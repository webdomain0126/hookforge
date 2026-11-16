use serde::Deserialize;
use thiserror::Error;

/// What a customer sends us when they register a new endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct NewEndpoint {
    pub url: String,
    pub secret: String,
    pub max_per_second: u32,
}

/// Reasons a NewEndpoint can be rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("url must start with https://")]
    UrlNotHttps,

    #[error("secret must be at least 16 characters")]
    SecretTooShort,

    #[error("max_per_second must be between 1 and 1000")]
    RateOutOfRange,
}

impl NewEndpoint {
    /// Checks the endpoint. Returns Ok(()) if it is valid,
    /// or the first mistake found.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.url.starts_with("https://") {
            return Err(ValidationError::UrlNotHttps);
        }

        if self.secret.len() < 16 {
            return Err(ValidationError::SecretTooShort);
        }

        if self.max_per_second < 1 || self.max_per_second > 1000 {
            return Err(ValidationError::RateOutOfRange);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid endpoint we can copy and break on purpose.
    fn good() -> NewEndpoint {
        NewEndpoint {
            url: "https://example.com/hook".to_string(),
            secret: "0123456789abcdef".to_string(),
            max_per_second: 10,
        }
    }

    #[test]
    fn good_endpoint_is_valid() {
        assert_eq!(good().validate(), Ok(()));
    }

    #[test]
    fn http_url_is_rejected() {
        let mut ep = good();
        ep.url = "http://example.com/hook".to_string();
        assert_eq!(ep.validate(), Err(ValidationError::UrlNotHttps));
    }

    #[test]
    fn short_secret_is_rejected() {
        let mut ep = good();
        ep.secret = "abc".to_string();
        assert_eq!(ep.validate(), Err(ValidationError::SecretTooShort));
    }

    #[test]
    fn zero_rate_is_rejected() {
        let mut ep = good();
        ep.max_per_second = 0;
        assert_eq!(ep.validate(), Err(ValidationError::RateOutOfRange));
    }
}