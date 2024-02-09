use scraper::{Html, Selector};

#[cfg(feature = "native-http")]
/// If the file provided is an html file, attempt to extract out the core content to improve
/// overall quality of UnstructuredElements returned.
pub fn extract_core_content(file_buffer: Vec<u8>, file_name: &str) -> Vec<u8> {
    if file_name.ends_with(".html") || file_name.ends_with(".htm") {
        let file_content = String::from_utf8_lossy(&file_buffer);
        let document = Html::parse_document(&file_content);

        // If the file is from GitHub, use a specific selector for GitHub's layout
        if file_name.contains("github.com") {
            if let Ok(layout_selector) = Selector::parse(".entry-content") {
                if let Some(layout_element) = document.select(&layout_selector).next() {
                    return layout_element.inner_html().into_bytes();
                }
            }
        } else if file_name.contains("twitter.com") || file_name.contains("x.com") {
            // Selector for Twitter or X.com's layout
            if let Ok(primary_column_selector) = Selector::parse("div[data-testid='primaryColumn']") {
                if let Some(primary_column_element) = document.select(&primary_column_selector).next() {
                    return primary_column_element.inner_html().into_bytes();
                }
            }
        } else if file_name.contains("youtube.com") {
            // Selector for YouTube's layout
            let mut content = String::new();
            if let Ok(above_the_fold_selector) = Selector::parse("#above-the-fold") {
                if let Some(above_the_fold_element) = document.select(&above_the_fold_selector).next() {
                    content += &above_the_fold_element.inner_html();
                }
            }
            if let Ok(comments_selector) = Selector::parse(".ytd-comments") {
                if let Some(comments_element) = document.select(&comments_selector).next() {
                    content += &comments_element.inner_html();
                }
            }
            return content.into_bytes();
        } else {
            // Try to select the 'main', 'article' tag or a class named 'main'
            if let Ok(main_selector) = Selector::parse("main, .main, article") {
                if let Some(main_element) = document.select(&main_selector).next() {
                    return main_element.inner_html().into_bytes();
                }
            }
        }
    }

    file_buffer
}
