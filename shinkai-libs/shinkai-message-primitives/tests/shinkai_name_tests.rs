#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

    #[test]
    fn test_valid_names() {
        println!("Testing valid names");
        let valid_names = vec![
            "@@alice.shinkai",
            "@@ALICE.SHINKAI",
            "@@alice_in_chains.shinkai",
            "@@alice/subidentity",
            "@@alice.shinkai/profileName",
            "@@alice.shinkai/profileName/agent/myChatGPTAgent",
            "@@alice.shinkai/profileName/device/myPhone",
            "@@alice.arb-sep-shinkai",
            "@@alice.arb-sep-shinkai/profileName",
            "@@alice.arb-sep-shinkai/profileName/agent/myChatGPTAgent",
            "@@alice.arb-sep-shinkai/profileName/device/myPhone",
            "@@_my_9552.arb-sep-shinkai/main",
        ];

        for name in valid_names {
            let result = ShinkaiName::new(name.to_string());
            assert!(result.is_ok(), "Expected {} to be valid, but it was not.", name);
        }
    }

    #[test]
    fn test_invalid_names_with_repair() {
        let invalid_names = vec![
            "@@alice.shinkai/profileName/myPhone",
            "@@alice-not-in-chains.shinkai",
            "@alice.shinkai",
            "@@@alice.shinkai",
            "@@al!ce.shinkai",
            "@@alice.shinkai//",
            "@@alice.shinkai//subidentity",
            "@@node1.shinkai/profile_1.shinkai",
        ];

        for name in invalid_names {
            let result = ShinkaiName::new(name.to_string());
            assert!(result.is_err(), "Expected {} to be invalid, but it was not.", name);
        }
    }

    #[test]
    fn test_invalid_names_without_repair() {
        let invalid_names = vec![
            "@@alice.shinkai/profileName/myPhone",
            "@@al!ce.shinkai",
            "@@alice/subidentity",
            "@@alice.shinkai//",
            "@@alice.shinkai//subidentity",
            "@@node1.shinkai/profile_1.shinkai",
        ];

        for name in invalid_names {
            let result = ShinkaiName::is_fully_valid(name.to_string());
            assert!(!result, "Expected {} to be invalid, but it was not.", name);
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
    fn test_from_node_and_profile_names_valid() {
        // Since the function can correct this, we just check for a valid response.
        let result = ShinkaiName::from_node_and_profile_names("bob.shinkai".to_string(), "profileBob".to_string());
        assert!(result.is_ok(), "Expected the name to be valid");
    }

    #[test]
    fn test_from_node_and_profile_names_invalid() {
        // If we want to ensure that the format isn't automatically fixed, we could use a clearly invalid name.
        let result = ShinkaiName::from_node_and_profile_names("b!ob".to_string(), "profileBob".to_string());
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
    fn test_get_profile_name_string() {
        let shinkai_name = ShinkaiName::new("@@frank.shinkai/profileFrank".to_string()).unwrap();
        assert_eq!(shinkai_name.get_profile_name_string(), Some("profilefrank".to_string()));

        let shinkai_name = ShinkaiName::new("@@frank.shinkai/profile_1/device/device_1".to_string()).unwrap();
        assert_eq!(shinkai_name.get_profile_name_string(), Some("profile_1".to_string()));
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

    #[test]
    fn test_contains() {
        let alice = ShinkaiName::new("@@alice.shinkai".to_string()).unwrap();
        let alice_profile = ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap();
        let alice_agent = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        let alice_device = ShinkaiName::new("@@alice.shinkai/profileName/device/myDevice".to_string()).unwrap();

        assert!(alice.contains(&alice_profile));
        assert!(alice.contains(&alice_agent));
        assert!(alice_profile.contains(&alice_agent));
        assert!(alice_profile.contains(&alice_profile));
        assert!(alice_profile.contains(&alice_device));

        assert!(!alice_profile.contains(&alice));
        assert!(!alice_device.contains(&alice_profile));
    }

    #[test]
    fn test_does_not_contain() {
        let alice = ShinkaiName::new("@@alice.shinkai".to_string()).unwrap();
        let bob = ShinkaiName::new("@@bob.shinkai".to_string()).unwrap();
        let alice_profile = ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap();
        let alice_agent = ShinkaiName::new("@@alice.shinkai/profileName/agent/bobsGPT".to_string()).unwrap();
        let bob_agent = ShinkaiName::new("@@bob.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();

        assert!(!alice.contains(&bob));
        assert!(!bob.contains(&alice));
        assert!(!alice_profile.contains(&bob));
        assert!(!bob.contains(&alice_profile));
        assert!(!alice_agent.contains(&bob_agent));
    }

    #[test]
    fn test_get_fullname_string_without_node_name() {
        let shinkai_name1 = ShinkaiName::new("@@alice.shinkai".to_string()).unwrap();
        assert_eq!(shinkai_name1.get_fullname_string_without_node_name(), None);

        let shinkai_name2 = ShinkaiName::new("@@alice.shinkai/profileName".to_string()).unwrap();
        assert_eq!(
            shinkai_name2.get_fullname_string_without_node_name(),
            Some("profilename".to_string())
        );

        let shinkai_name3 = ShinkaiName::new("@@alice.shinkai/profileName/agent/myChatGPTAgent".to_string()).unwrap();
        assert_eq!(
            shinkai_name3.get_fullname_string_without_node_name(),
            Some("profilename/agent/mychatgptagent".to_string())
        );

        let shinkai_name4 = ShinkaiName::new("@@alice.shinkai/profileName/device/myPhone".to_string()).unwrap();
        assert_eq!(
            shinkai_name4.get_fullname_string_without_node_name(),
            Some("profilename/device/myphone".to_string())
        );
    }
}
