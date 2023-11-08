use std::fmt;

use reqwest::get;
use scraper::{Html, Selector};

#[derive(Debug)]
pub enum YoutubeCheckerError {
    RequestError(reqwest::Error),
    HtmlParseError(String),
    InvalidUrlError,
}

impl fmt::Display for YoutubeCheckerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            YoutubeCheckerError::RequestError(err) => write!(f, "Request error: {}", err),
            YoutubeCheckerError::HtmlParseError(err) => write!(f, "HTML parse error: {}", err),
            YoutubeCheckerError::InvalidUrlError => write!(f, "Invalid URL"),
        }
    }
}

impl std::error::Error for YoutubeCheckerError {}

pub struct YoutubeChecker {}

impl YoutubeChecker {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn check_new_videos(&self, url: &str) -> Result<bool, YoutubeCheckerError> {
        // Check if the URL is a YouTube channel's video list
        if url.starts_with("https://www.youtube.com/") && url.contains("/videos") {
            let resp = get(url)
                .await
                .map_err(|err| YoutubeCheckerError::RequestError(err))?;

            let document = Html::parse_document(
                &resp
                    .text()
                    .await
                    .map_err(|err| YoutubeCheckerError::RequestError(err))?,
            );

            eprintln!("{:?}", document);

            // Create a Selector to find the video links
            let video_selector = Selector::parse("a.yt-simple-endpoint.style-scope.ytd-grid-video-renderer").unwrap();
            eprintln!("{:?}", video_selector);

            // Extract the video links
            let mut video_links: Vec<String> = Vec::new();
            for element in document.select(&video_selector) {
                if let Some(video_link) = element.value().attr("href") {
                    video_links.push(video_link.to_string());
                }
            }

            // TODO: Compare these links with the ones stored from the last cron check
            // Note(Nico): can we have deterministic video IDs so we know if we already reviewed it them?
            // to see if there are any new videos
        } else {
            return Err(YoutubeCheckerError::InvalidUrlError);
        }

        Ok(true)
    }
}
