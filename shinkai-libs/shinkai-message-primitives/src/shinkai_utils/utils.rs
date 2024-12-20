    /// Cleans an input string to ensure that it does not have any
    /// characters which would break a VRPath, or cause issues generally for the VectorFS.
    pub fn clean_string(s: &str) -> String {
        s.replace("/", "-").replace(":", "_")
    }