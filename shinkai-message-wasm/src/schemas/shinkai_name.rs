use std::fmt;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShinkaiName(String);

// Name Examples
// @@alice.shinkai
// @@alice.shinkai/profileName
// @@aAlice.shinkai/profileName/myChatGPTAgent
// @@alice.shinkai/profileName/myPhone

impl ShinkaiName {
    pub fn new(raw_name: String) -> Result<Self, &'static str> {
        // Check if it contains '@@', '.shinkai', and '/'
        if !raw_name.contains("@@") || !raw_name.contains(".shinkai") || !raw_name.contains("/") {
            return Err("Invalid name format.");
        }

        // Split by '/' and check if it has two or three parts: node, profile, and optional device
        let parts: Vec<&str> = raw_name.split('/').collect();
        if !(parts.len() == 2 || parts.len() == 3) {
            return Err("Name should have two or three parts: node, profile, and optional device.");
        }

        // Check if the node part starts with '@@' and ends with '.shinkai'
        if !parts[0].starts_with("@@") || !parts[0].ends_with(".shinkai") {
            return Err("Node part of the name should start with '@@' and end with '.shinkai'.");
        }

        // If all checks passed, create a new ShinkaiName instance
        Ok(Self(raw_name.to_lowercase()))
    }

    pub fn from_node_and_profile(node_name: String, profile_name: String) -> Result<Self, &'static str> {
        // Validate and format the node_name
        let node_name = if Self::is_valid_node_identity_name_and_no_subidentities(&node_name) {
            node_name
        } else {
            format!("@@{}.shinkai", node_name)
        };

        // Construct the full_identity_name
        let full_identity_name = format!("{}/{}", node_name.to_lowercase(), profile_name.to_lowercase());

        // Create a new ShinkaiName
        Self::new(full_identity_name)
    }

    // This method checks if a name is a valid node identity name and doesn't contain subidentities
    fn is_valid_node_identity_name_and_no_subidentities(name: &String) -> bool {
        // A node name is valid if it starts with '@@', ends with '.shinkai', and doesn't contain '/'
        name.starts_with("@@") && name.ends_with(".shinkai") && !name.contains("/")
    }

    pub fn has_profile(&self) -> bool {
        // Check if it contains two '/' (indicating it has a profile)
        self.0.matches('/').count() >= 1
    }

    pub fn has_device(&self) -> bool {
        // Check if it contains two '/' (indicating it has a device)
        self.0.matches('/').count() == 2
    }

    pub fn has_no_subidentities(&self) -> bool {
        // If it contains no '/' then it's only a node
        !self.0.contains('/')
    }

    pub fn extract_profile(&self) -> Result<Self, &'static str> {
        if self.has_no_subidentities() {
            return Err("This ShinkaiName does not include a profile.");
        }

        let parts: Vec<&str> = self.0.splitn(2, '/').collect();
        // parts[0] now contains the node name with '@@' and '.shinkai', and parts[1] contains the profile name

        // Form a new ShinkaiName with only the node and profile, but no device
        Self::new(format!("{}/{}", parts[0], parts[1]))
    }

    pub fn extract_node(&self) -> Result<Self, &'static str> {
        let parts: Vec<&str> = self.0.split('/').collect();
        // parts[0] now contains the node name with '@@' and '.shinkai'

        // Return a new ShinkaiName with only the node
        Self::new(parts[0].to_string())
    }
}

impl fmt::Display for ShinkaiName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<[u8]> for ShinkaiName {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
