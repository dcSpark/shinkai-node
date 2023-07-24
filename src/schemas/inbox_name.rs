use serde::{Serialize, Deserialize};
use crate::db::db_errors::ShinkaiDBError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InboxName {
    pub value: String,
}

impl InboxName {
    pub fn new(s: String) -> Result<Self, ShinkaiDBError> {
        Self::from_str(&s)
    }

    fn from_str(s: &str) -> Result<Self, ShinkaiDBError> {
        let parts: Vec<&str> = s.split("::").collect();
        if parts.len() != 4 {
            return Err(ShinkaiDBError::InvalidInboxName);
        }

        let is_e2e = match parts[3].parse::<bool>() {
            Ok(b) => b,
            Err(_) => return Err(ShinkaiDBError::InvalidInboxName),
        };

        let sender_parts: Vec<&str> = parts[1].split("|").collect();
        let recipient_parts: Vec<&str> = parts[2].split("|").collect();

        if sender_parts.len() != 2 || recipient_parts.len() != 2 {
            return Err(ShinkaiDBError::InvalidInboxName);
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
