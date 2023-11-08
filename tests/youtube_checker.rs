
#[cfg(test)]
mod tests {
    use shinkai_node::cron_tasks::youtube_checker::YoutubeChecker;

    use super::*;

    #[tokio::test]
    async fn test_check_new_videos() {
        let youtube_checker = YoutubeChecker::new();
        let url = "https://www.youtube.com/@ZeihanonGeopolitics/videos";
        let result = youtube_checker.check_new_videos(url).await;
        eprintln!("{:?}", result);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }
}