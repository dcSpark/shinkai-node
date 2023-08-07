
#[derive(Debug)]
pub enum NodeError {
    InStandbyMode,
    Io(std::io::Error),
    Other(String),
    // ... other error variants you might want ...
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NodeError::InStandbyMode => write!(f, "Node is in standby mode"),
            NodeError::Io(err) => write!(f, "IO error: {}", err),
            NodeError::Other(msg) => write!(f, "{}", msg),
            // ... handle other variants here ...
        }
    }
}

impl std::error::Error for NodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NodeError::Io(err) => Some(err),
            // Other variants don't wrap other errors, so we return None.
            _ => None,
        }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for NodeError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> NodeError {
        NodeError::Other(format!("{}", err))
    }
}

impl From<std::io::Error> for NodeError {
    fn from(err: std::io::Error) -> NodeError {
        NodeError::Io(err)
    }
}
