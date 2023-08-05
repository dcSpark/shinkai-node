use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_names() {
        println!("Testing valid names");
        let valid_names = vec![
            "@@alice.shinkai",
            "@@alice.shinkai/profileName",
            "@@alice.shinkai/profileName/agent/myChatGPTAgent",
            "@@alice.shinkai/profileName/device/myPhone",
        ];

        for name in valid_names {
            let result = ShinkaiName::new(name.to_string());
            assert!(result.is_ok(), "Expected {} to be valid, but it was not.", name);
        }
    }

    #[test]
    fn test_invalid_names() {
        let invalid_names = vec![
            "@@alice.shinkai/profileName/myPhone",
            "@@al!ce.shinkai",
            "@@alice.shinkai//",
            "@@node1.shinkai/profile_1.shinkai",
        ];

        for name in invalid_names {
            let result = ShinkaiName::new(name.to_string());
            assert!(result.is_err(), "Expected {} to be invalid, but it was not.", name);
        }
    }

    #[test]
    fn test_no_shinkai_suffix() {
        let name = "@@alice";
        let result = ShinkaiName::new(name.to_string());
        assert!(result.is_ok(), "Expected the name to be formatted correctly");
        assert_eq!(result.unwrap().to_string(), "@@alice.shinkai");
    }

    #[test]
    fn test_no_shinkai_prefix() {
        let name = "alice.shinkai";
        let result = ShinkaiName::new(name.to_string());
        assert!(result.is_ok(), "Expected the name to be formatted correctly");
        assert_eq!(result.unwrap().to_string(), "@@alice.shinkai");
    }

    #[test]
    fn test_from_node_and_profile_valid() {
        // Since the function can correct this, we just check for a valid response.
        let result = ShinkaiName::from_node_and_profile("bob.shinkai".to_string(), "profileBob".to_string());
        println!("Result: {:?}", result);
        assert!(result.is_ok(), "Expected the name to be valid");
    }

    #[test]
    fn test_from_node_and_profile_invalid() {
        // If we want to ensure that the format isn't automatically fixed, we could use a clearly invalid name.
        let result = ShinkaiName::from_node_and_profile("b!ob".to_string(), "profileBob".to_string());
        assert!(result.is_err(), "Expected the name to be invalid");
    }

    #[test]
    fn test_has_profile() {
        let shinkai_name = ShinkaiName::new("@@charlie.shinkai/profileCharlie".to_string()).unwrap();
        assert!(shinkai_name.has_profile());
    }

    #[test]
    fn test_has_device() {
        let shinkai_name = ShinkaiName::new("@@dave.shinkai/profileDave/device/myDevice".to_string()).unwrap();
        assert!(shinkai_name.has_device());
    }

    #[test]
    fn test_has_no_subidentities() {
        let shinkai_name = ShinkaiName::new("@@eve.shinkai".to_string()).unwrap();
        assert!(!shinkai_name.has_profile(), "Name shouldn't have a profile");
        assert!(!shinkai_name.has_device(), "Name shouldn't have a device");
        assert!(shinkai_name.has_no_subidentities(), "Name should have no subidentities");
    }

    #[test]
    fn test_get_profile_name() {
        let shinkai_name = ShinkaiName::new("@@frank.shinkai/profileFrank".to_string()).unwrap();
        assert_eq!(shinkai_name.get_profile_name(), Some("profilefrank".to_string()));
    }

    #[test]
    fn test_extract_profile() {
        let shinkai_name = ShinkaiName::new("@@frank.shinkai/profileFrank".to_string()).unwrap();
        let extracted = shinkai_name.extract_profile();
        assert!(extracted.is_ok(), "Extraction should be successful");
        assert_eq!(extracted.unwrap().to_string(), "@@frank.shinkai/profilefrank");
    }

    #[test]
    fn test_extract_node() {
        let shinkai_name = ShinkaiName::new("@@henry.shinkai/profileHenry/device/myDevice".to_string()).unwrap();
        let node = shinkai_name.extract_node();
        assert_eq!(node.to_string(), "@@henry.shinkai");
    }
}
