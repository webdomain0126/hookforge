use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Delivering,
    Delivered,
    Dead,
}

impl DeliveryStatus {
    /// A terminal delivery is finished and never changes again.
    pub fn is_terminal(&self) -> bool {
        match self {
            DeliveryStatus::Pending => false,
            DeliveryStatus::Delivering => false,
            DeliveryStatus::Delivered => true,
            DeliveryStatus::Dead => true,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivered_is_terminal() {
        assert!(DeliveryStatus::Delivered.is_terminal());
    }

    #[test]
    fn pending_is_not_terminal() {
        assert!(!DeliveryStatus::Pending.is_terminal());
    }

    #[test]
    fn dead_is_terminal() {
        assert!(DeliveryStatus::Dead.is_terminal());
    }

    #[test]
    fn delivering_is_not_terminal() {
        assert!(!DeliveryStatus::Delivering.is_terminal());
    }
}