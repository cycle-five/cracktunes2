use async_trait::async_trait;
use super::audio_player::GuildId;
use std::time::Duration;

/// Error type for StateManager operations
#[derive(Debug, thiserror::Error)]
pub enum StateManagerError {
    #[error("Key not found")]
    KeyNotFound,
    
    #[error("Failed to store value: {0}")]
    StoreError(String),
    
    #[error("Failed to retrieve value: {0}")]
    RetrieveError(String),
    
    #[error("Failed to delete value: {0}")]
    DeleteError(String),
    
    #[error("Value type mismatch")]
    TypeError,
    
    #[error("State manager error: {0}")]
    Other(String),
}

/// Result type for StateManager operations
pub type StateManagerResult<T> = Result<T, StateManagerError>;

/// Guild-specific configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuildConfig {
    pub idle_timeout_minutes: Option<u64>, // None means no timeout
    pub default_volume: f32,
    pub announce_tracks: bool,
    pub prefix: Option<String>,
}

impl Default for GuildConfig {
    fn default() -> Self {
        Self {
            idle_timeout_minutes: Some(5), // Default 5 minutes timeout
            default_volume: 0.5,           // 50% volume
            announce_tracks: true,
            prefix: None,                  // Use default prefix
        }
    }
}

/// Interface for managing application state
#[async_trait]
pub trait StateManager: Send + Sync + 'static {
    /// Store a value with a key
    async fn set<T: serde::Serialize + Send + Sync + 'static>(
        &self, 
        key: &str, 
        value: &T
    ) -> StateManagerResult<()>;
    
    /// Retrieve a value by key
    async fn get<T: serde::de::DeserializeOwned + Send + Sync + 'static>(
        &self, 
        key: &str
    ) -> StateManagerResult<Option<T>>;
    
    /// Delete a value by key
    async fn delete(&self, key: &str) -> StateManagerResult<()>;
    
    /// Get the configuration for a guild
    async fn get_guild_config(&self, guild_id: GuildId) -> StateManagerResult<GuildConfig> {
        let key = format!("guild:{}:config", guild_id.0);
        match self.get::<GuildConfig>(&key).await? {
            Some(config) => Ok(config),
            None => Ok(GuildConfig::default()),
        }
    }
    
    /// Set the configuration for a guild
    async fn set_guild_config(&self, guild_id: GuildId, config: &GuildConfig) -> StateManagerResult<()> {
        let key = format!("guild:{}:config", guild_id.0);
        self.set(&key, config).await
    }
    
    /// Set the idle timeout for a guild
    async fn set_idle_timeout(&self, guild_id: GuildId, timeout: Option<Duration>) -> StateManagerResult<()> {
        let mut config = self.get_guild_config(guild_id).await?;
        config.idle_timeout_minutes = timeout.map(|t| t.as_secs() / 60);
        self.set_guild_config(guild_id, &config).await
    }
    
    /// Get the idle timeout for a guild
    async fn get_idle_timeout(&self, guild_id: GuildId) -> StateManagerResult<Option<Duration>> {
        let config = self.get_guild_config(guild_id).await?;
        Ok(config.idle_timeout_minutes.map(|mins| Duration::from_secs(mins * 60)))
    }
    
    /// Update the last activity timestamp for a guild
    async fn update_last_activity(&self, guild_id: GuildId) -> StateManagerResult<()> {
        let key = format!("guild:{}:last_activity", guild_id.0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.set(&key, &now).await
    }
    
    /// Get the last activity timestamp for a guild
    async fn get_last_activity(&self, guild_id: GuildId) -> StateManagerResult<Option<u64>> {
        let key = format!("guild:{}:last_activity", guild_id.0);
        self.get::<u64>(&key).await
    }
}