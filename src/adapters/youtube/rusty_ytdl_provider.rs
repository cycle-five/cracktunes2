use crate::core::ports::audio_provider::{
    AudioMetadata, AudioProvider, AudioProviderError, AudioSearchResult, AudioStream,
};
use async_trait::async_trait;
use rusty_ytdl::{
    search::{SearchOptions, SearchResult, YouTube},
    RequestOptions, Video, VideoOptions,
};
use std::sync::Arc;

/// Implementation of AudioProvider using rusty_ytdl
pub struct RustyYtdlProvider {
    client: reqwest::Client,
    ytdl: YouTube,
}

impl RustyYtdlProvider {
    /// Create a new instance with a custom reqwest client
    pub fn new(client: reqwest::Client) -> Result<Self, AudioProviderError> {
        let request_options = RequestOptions {
            client: Some(client.clone()),
            ..Default::default()
        };

        let ytdl = YouTube::new_with_options(&request_options)
            .map_err(|e| AudioProviderError::SearchError(e.to_string()))?;

        Ok(Self { client, ytdl })
    }

    /// Create a new instance with default reqwest client
    pub fn new_default() -> Result<Self, AudioProviderError> {
        Self::new(reqwest::Client::new())
    }

    // Helper method to convert rusty_ytdl::search::SearchResult to our domain model
    fn convert_search_result(result: SearchResult) -> AudioSearchResult {
        match result {
            SearchResult::Video(video) => {
                AudioSearchResult {
                    title: video.title,
                    url: video.url,
                    duration: Some(format!("{}:{:02}", video.duration / 60, video.duration % 60)),
                    artist: None, // YouTube doesn't consistently provide artist info
                    thumbnail_url: video.thumbnails.first().map(|t| t.url.clone()),
                }
            }
            SearchResult::Playlist(playlist) => {
                AudioSearchResult {
                    title: format!("Playlist: {}", playlist.name),
                    url: playlist.url,
                    duration: None, // Playlists don't have durations
                    artist: None,
                    thumbnail_url: None, // Could get from first video, but complex
                }
            }
            SearchResult::Channel(channel) => AudioSearchResult {
                title: format!("Channel: {}", channel.name),
                url: channel.url,
                duration: None,
                artist: None,
                thumbnail_url: None,
            },
        }
    }

    // Helper method to convert rusty_ytdl::VideoInfo to our AudioMetadata
    fn video_info_to_metadata(info: &rusty_ytdl::VideoInfo) -> AudioMetadata {
        AudioMetadata {
            title: Some(info.video_details.title.clone()),
            artist: match &info.video_details.author {
                Some(author) => Some(author.name.clone()),
                None => Some("Unknown".to_string()),
            },
            album: None,
            duration: {
                // Convert length_seconds to a duration string
                let length_seconds = &info.video_details.length_seconds;

                // Parse the seconds and format as MM:SS
                if let Ok(seconds) = length_seconds.parse::<u64>() {
                    let minutes = seconds / 60;
                    let remaining_seconds = seconds % 60;
                    Some(format!("{}:{:02}", minutes, remaining_seconds))
                } else {
                    None
                }
            },
            source_url: Some(info.video_details.video_url.clone()),
            thumbnail_url: info.video_details.thumbnails.first().map(|t| t.url.clone()),
        }
    }
}

#[async_trait]
impl AudioProvider for RustyYtdlProvider {
    async fn search(&self, query: &str) -> Result<Vec<AudioSearchResult>, AudioProviderError> {
        let search_options = SearchOptions {
            limit: 5,
            ..Default::default()
        };

        let results = self
            .ytdl
            .search(query, Some(&search_options))
            .await
            .map_err(|e| AudioProviderError::SearchError(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(Self::convert_search_result)
            .collect())
    }

    async fn get_suggestions(
        &self,
        partial_query: &str,
    ) -> Result<Vec<String>, AudioProviderError> {
        if partial_query.len() < 3 {
            return Ok(Vec::new());
        }

        self.ytdl
            .suggestion(
                partial_query.to_string(),
                Some(rusty_ytdl::search::LanguageTags::EN),
            )
            .await
            .map_err(|e| AudioProviderError::SearchError(e.to_string()))
    }

    async fn get_stream(&self, url: &str) -> Result<AudioStream, AudioProviderError> {
        // Create video options with our client
        let video_options = VideoOptions {
            request_options: RequestOptions {
                client: Some(self.client.clone()),
                ..Default::default()
            },
            ..Default::default()
        };

        // Create the video object
        let video = Video::new_with_options(url, video_options)
            .map_err(|e| AudioProviderError::InvalidUrl(e.to_string()))?;

        // Get video info for metadata
        let video_info = video
            .get_basic_info()
            .await
            .map_err(|e| AudioProviderError::StreamError(e.to_string()))?;

        // Convert to our metadata model
        let metadata = Self::video_info_to_metadata(&video_info);

        // The actual stream will be handled by the audio player adapter
        Ok(AudioStream {
            metadata: Some(metadata),
            provider_data: Arc::new(video), // Store the video object for later use
        })
    }

    fn can_handle_url(&self, url: &str) -> bool {
        url.contains("youtube.com")
            || url.contains("youtu.be")
            || url.contains("youtube-nocookie.com")
    }
}

pub fn build_mock_rusty_ytdl_video() -> rusty_ytdl::search::Video {
    rusty_ytdl::search::Video {
        id: "test-id".to_string(),
        title: "Test Video".to_string(),
        url: "https://youtube.com/watch?v=test-id".to_string(),
        description: "Test description".to_string(),
        duration_raw: "180".to_string(),
        duration: 180,
        thumbnails: vec![rusty_ytdl::Thumbnail {
            url: "https://example.com/thumbnail.jpg".to_string(),
            width: 120,
            height: 90,
        }],
        channel: rusty_ytdl::search::Channel {
            id: "channel-id".to_string(),
            name: "Test Channel".to_string(),
            url: "https://youtube.com/channel/channel-id".to_string(),
            icon: Vec::new(),
            verified: false,
            subscribers: 0,
        },
        uploaded_at: None,
        views: 0,
    }
}

// Unit tests for the RustyYtdlProvider
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;

    // Create a mock for the AudioProvider trait instead
    mock! {
        pub AudioProviderMock {}
        #[async_trait]
        impl AudioProvider for AudioProviderMock {
            async fn search(&self, query: &str) -> Result<Vec<AudioSearchResult>, AudioProviderError>;
            async fn get_suggestions(&self, partial_query: &str) -> Result<Vec<String>, AudioProviderError>;
            async fn get_stream(&self, url: &str) -> Result<AudioStream, AudioProviderError>;
            fn can_handle_url(&self, url: &str) -> bool;
        }
    }

    #[test]
    fn test_can_handle_url() {
        let client = reqwest::Client::new();
        let provider = RustyYtdlProvider::new(client).unwrap();

        assert!(provider.can_handle_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(provider.can_handle_url("https://youtu.be/dQw4w9WgXcQ"));
        assert!(provider.can_handle_url("https://www.youtube-nocookie.com/watch?v=dQw4w9WgXcQ"));
        assert!(!provider.can_handle_url("https://www.example.com/video"));
    }

    #[test]
    fn test_convert_search_result() {
        // Test converting a video result
        let video_result = SearchResult::Video(build_mock_rusty_ytdl_video());

        let domain_result = RustyYtdlProvider::convert_search_result(video_result);

        assert_eq!(domain_result.title, "Test Video");
        assert_eq!(domain_result.url, "https://youtube.com/watch?v=test-id");
        assert_eq!(domain_result.duration, Some("3:00".to_string()));
        assert_eq!(
            domain_result.thumbnail_url,
            Some("https://example.com/thumbnail.jpg".to_string())
        );
    }
}
