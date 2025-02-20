// Define the IndexableVersion struct
#[derive(Debug, Eq, PartialEq)]
pub struct IndexableVersion {
    version_number: u64,
}

// Implement comparison traits
impl PartialOrd for IndexableVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexableVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version_number.cmp(&other.version_number)
    }
}

// Implement Display trait for string formatting
impl std::fmt::Display for IndexableVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_version_string())
    }
}

impl IndexableVersion {
    // Constructor that takes a version string
    pub fn from_string(version: &str) -> Result<Self, String> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() > 3 {
            return Err("Version string can have at most 3 parts".to_string());
        }

        let mut version_number = 0;
        for (i, part) in parts.iter().enumerate() {
            let num: u64 = part
                .parse()
                .map_err(|_| "Invalid number in version string".to_string())?;

            // Check if the number is above 999 and not in the first position
            if i > 0 && num > 999 {
                return Err("Numbers above 999 are only allowed in the first position".to_string());
            }

            let factor = match i {
                0 => 1_000_000,
                1 => 1_000,
                2 => 1,
                _ => unreachable!(),
            };

            version_number += num * factor;
        }

        Ok(IndexableVersion { version_number })
    }

    // Constructor that takes a version number directly
    pub fn from_number(version_number: u64) -> Self {
        IndexableVersion { version_number }
    }

    // Method to get the version number
    pub fn get_version_number(&self) -> u64 {
        self.version_number
    }

    // Method to convert the version number back to a version string
    pub fn to_version_string(&self) -> String {
        let major = self.version_number / 1_000_000;
        let minor = (self.version_number % 1_000_000) / 1_000;
        let patch = self.version_number % 1_000;
        format!("{}.{}.{}", major, minor, patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string_valid() {
        let version = IndexableVersion::from_string("1.2.3").unwrap();
        assert_eq!(version.get_version_number(), 1_002_003);

        let version = IndexableVersion::from_string("1.2").unwrap();
        assert_eq!(version.get_version_number(), 1_002_000);

        let version = IndexableVersion::from_string("1").unwrap();
        assert_eq!(version.get_version_number(), 1_000_000);

        // Test for "1.0"
        let version = IndexableVersion::from_string("1.0").unwrap();
        assert_eq!(version.get_version_number(), 1_000_000);

        // Test for "2.0"
        let version = IndexableVersion::from_string("2.0").unwrap();
        assert_eq!(version.get_version_number(), 2_000_000);
    }

    #[test]
    fn test_from_string_invalid() {
        assert!(IndexableVersion::from_string("1.2.3.4").is_err());
        assert!(IndexableVersion::from_string("1.a.3").is_err());
    }

    #[test]
    fn test_from_number() {
        let version = IndexableVersion::from_number(1_002_003);
        assert_eq!(version.get_version_number(), 1_002_003);
    }

    #[test]
    fn test_to_version_string() {
        let version = IndexableVersion::from_number(1_002_003);
        assert_eq!(version.to_version_string(), "1.2.3");

        let version = IndexableVersion::from_number(1_002_000);
        assert_eq!(version.to_version_string(), "1.2.0");

        let version = IndexableVersion::from_number(1_000_000);
        assert_eq!(version.to_version_string(), "1.0.0");

        let version = IndexableVersion::from_number(3);
        assert_eq!(version.to_version_string(), "0.0.3");
    }

    #[test]
    fn test_from_string_invalid_large_number() {
        // Test with a number above 999 in the second position
        assert!(IndexableVersion::from_string("1.1000.3").is_err());

        // Test with a number above 999 in the third position
        assert!(IndexableVersion::from_string("1.2.1000").is_err());

        // Test with a number above 999 in the first position (should be valid)
        assert!(IndexableVersion::from_string("1000.2.3").is_ok());
    }
}
