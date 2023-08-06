use std::fmt;

use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq)]
pub enum InboxNameError {
    InvalidFormat,
    InvalidSenderRecipientFormat,
}

impl fmt::Display for InboxNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InboxNameError::InvalidFormat => write!(f, "Invalid inbox name format"),
            InboxNameError::InvalidSenderRecipientFormat => write!(f, "Invalid sender/recipient format"),
        }
    }
}

impl std::error::Error for InboxNameError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InboxName {
    pub value: String,
}

impl InboxName {
    pub fn new(s: String) -> Result<Self, InboxNameError> {
        Self::from_str(&s)
    }

    fn from_str(s: &str) -> Result<Self, InboxNameError> {
        let parts: Vec<&str> = s.split("::").collect();
        if parts.len() != 4 {
            return Err(InboxNameError::InvalidFormat);
        }

        let is_e2e = match parts[3].parse::<bool>() {
            Ok(b) => b,
            Err(_) => return Err(InboxNameError::InvalidFormat),
        };

        let sender_parts: Vec<&str> = parts[1].split("|").collect();
        let recipient_parts: Vec<&str> = parts[2].split("|").collect();

        if sender_parts.len() != 2 || recipient_parts.len() != 2 {
            return Err(InboxNameError::InvalidFormat);
        }

        Ok(InboxName { value: s.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbox_name_valid() {
        let name = "part1|part2::part3|part4::part5|part6::true".to_string();
        assert!(InboxName::new(name).is_ok());
    }

    #[test]
    fn test_inbox_real_name_valid() {
        let name = "inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string();
        assert!(InboxName::new(name).is_ok());
    }

    #[test]
    fn test_inbox_name_invalid() {
        let name1 = "part1|part2::part3::part4::false".to_string();
        assert!(InboxName::new(name1).is_err());

        let name2 = "part1|part2::part3|part4::part5|part6::maybe".to_string();
        assert!(InboxName::new(name2).is_err());

        let name3 = "part1|part2|part3|part4::part5|part6::true".to_string();
        assert!(InboxName::new(name3).is_err());

        let name4 = "part1::part2|part3::part4|part5|part6::false".to_string();
        assert!(InboxName::new(name4).is_err());
    }
}
