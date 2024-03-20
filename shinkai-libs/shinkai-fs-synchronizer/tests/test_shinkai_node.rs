#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_file_synchronizer::communication::node_init;
    use shinkai_file_synchronizer::shinkai_manager::ShinkaiManager;

    #[tokio::test]
    async fn test_get_node_folder() {
        dotenv::dotenv().ok();
        let mut shinkai_manager = node_init().await.expect("Node initialization failed");

        let path = "/test_folder";

        let result = shinkai_manager.get_node_folder(path).await;

        assert!(result.is_ok(), "Failed to get node folder");
        let folder_path = result.unwrap();
        assert_eq!(
            folder_path, "/test_folder",
            "The returned folder path does not match the expected path"
        );
    }
}
