use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_valid() {
        assert!(ShinkaiName::new("@@alice.shinkai/profileName".to_string()).is_ok());
        assert!(ShinkaiName::new("@@alice.shinkai/profileName/myDevice".to_string()).is_ok());
    }

    #[test]
    fn test_new_invalid() {
        assert!(ShinkaiName::new("@@alice.shinkai".to_string()).is_err());
        assert!(ShinkaiName::new("alice.shinkai/profileName".to_string()).is_err());
        assert!(ShinkaiName::new("@@alice.shinkai/profileName/myDevice/extra".to_string()).is_err());
    }

    #[test]
    fn test_from_node_and_profile_valid() {
        assert!(ShinkaiName::from_node_and_profile("@@bob.shinkai".to_string(), "profileBob".to_string()).is_ok());
    }

    #[test]
    fn test_from_node_and_profile_invalid() {
        assert!(ShinkaiName::from_node_and_profile("bob.shinkai".to_string(), "profileBob".to_string()).is_err());
    }

    #[test]
    fn test_has_profile() {
        let shinkai_name = ShinkaiName::new("@@charlie.shinkai/profileCharlie".to_string()).unwrap();
        assert!(shinkai_name.has_profile());
    }

    #[test]
    fn test_has_device() {
        let shinkai_name = ShinkaiName::new("@@dave.shinkai/profileDave/myDevice".to_string()).unwrap();
        assert!(shinkai_name.has_device());
    }

    #[test]
    fn test_has_no_subidentities() {
        let shinkai_name = ShinkaiName::new("@@eve.shinkai".to_string()).unwrap();
        assert!(shinkai_name.has_no_subidentities());
    }

    #[test]
    fn test_get_profile_name() {
        let shinkai_name = ShinkaiName::new("@@frank.shinkai/profileFrank".to_string()).unwrap();
        assert_eq!(shinkai_name.get_profile_name(), Some("profileFrank".to_string()));
    }

    #[test]
    fn test_extract_profile() {
        let shinkai_name = ShinkaiName::new("@@george.shinkai/profileGeorge/myDevice".to_string()).unwrap();
        let expected_shinkai_name = ShinkaiName::new("@@george.shinkai/profileGeorge".to_string()).unwrap();
        assert_eq!(shinkai_name.extract_profile().unwrap(), expected_shinkai_name);
    }

    #[test]
    fn test_extract_node() {
        let shinkai_name = ShinkaiName::new("@@henry.shinkai/profileHenry/myDevice".to_string()).unwrap();
        let expected_shinkai_name = ShinkaiName::new("@@henry.shinkai".to_string()).unwrap();
        assert_eq!(shinkai_name.extract_node(), expected_shinkai_name);
    }
}
