use core::fmt;
use std::{error::Error, str::FromStr};

#[derive(Debug, PartialEq, PartialOrd)]
pub enum InboxPermission {
    Read,  // it contains None
    Write, // it contains Read
    Admin, // it contains Write
}

impl InboxPermission {
    pub fn to_i32(&self) -> i32 {
        match self {
            InboxPermission::Read => 1,
            InboxPermission::Write => 2,
            InboxPermission::Admin => 3,
        }
    }

    pub fn from_i32(val: i32) -> Result<Self, Box<dyn Error>> {
        match val {
            1 => Ok(InboxPermission::Read),
            2 => Ok(InboxPermission::Write),
            3 => Ok(InboxPermission::Admin),
            _ => Err(Box::<dyn Error>::from(format!("Invalid permission string: {}", val))),
        }
    }
}

impl fmt::Display for InboxPermission {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InboxPermission::Read => write!(f, "Read"),
            InboxPermission::Write => write!(f, "Write"),
            InboxPermission::Admin => write!(f, "Admin"),
        }
    }
}

impl FromStr for InboxPermission {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Read" => Ok(InboxPermission::Read),
            "Write" => Ok(InboxPermission::Write),
            "Admin" => Ok(InboxPermission::Admin),
            _ => Err(Box::<dyn Error + Send + Sync>::from(
                format!("Invalid permission string: {}", s).to_string(),
            )),
        }
    }
}
