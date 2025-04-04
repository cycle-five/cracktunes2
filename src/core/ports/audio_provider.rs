use async_trait::async_trait;
use std::sync::Arc;

/// Domain model for a search result from an audio provider
#[derive(Debug, Clone)]
pub struct AudioSearchResult {
    pub title: String,
    pub url: String,
    pub duration: Option<String>,
    pub artist: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// Domain model for audio metadata
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<String>,
    pub source_url: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// Domain model for audio stream
#[derive(Debug, Clone)]
pub struct AudioStream {
    pub metadata: Option<AudioMetadata>,
    // The actual audio data implementation is left to the adapter
    pub provider_data: Arc<dyn std::any::Any + Send + Sync>,
}

/// Error type for AudioProvider operations
#[derive(Debug, thiserror::Error)]
pub enum AudioProviderError {
    #[error("Failed to search: {0}")]
    SearchError(String),
    
    #[error("Failed to retrieve stream: {0}")]
    StreamError(String),
    
    #[error("Audio source not found")]
    NotFound,
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("Authentication required")]
    AuthenticationRequired,
    
    #[error("Rate limited")]
    RateLimited,
}

/// Provides audio content from external sources like YouTube, Spotify, etc.
#[async_trait]
pub trait AudioProvider: Send + Sync + 'static {
    /// Search for audio content with the given query string
    async fn search(&self, query: &str) -> Result<Vec<AudioSearchResult>, AudioProviderError>;
    
    /// Get suggestions for a partial search query (for autocomplete)
    async fn get_suggestions(&self, partial_query: &str) -> Result<Vec<String>, AudioProviderError>;
    
    /// Get audio stream from a URL
    async fn get_stream(&self, url: &str) -> Result<AudioStream, AudioProviderError>;
    
    /// Get audio stream from a search query (performs search first, then gets stream)
    async fn get_stream_from_query(&self, query: &str) -> Result<AudioStream, AudioProviderError> {
        let results = self.search(query).await?;
        if results.is_empty() {
            return Err(AudioProviderError::NotFound);
        }
        self.get_stream(&results[0].url).await
    }
    
    /// Check if this provider can handle the given URL
    fn can_handle_url(&self, url: &str) -> bool;
}