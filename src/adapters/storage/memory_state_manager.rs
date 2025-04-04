use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

use crate::core::ports::state_manager::{StateManager, StateManagerError, StateManagerResult};

/// In-memory implementation of StateManager for development and testing
#[derive(Debug)]
pub struct MemoryStateManager {
    data: Arc<DashMap<String, String>>,
}

impl MemoryStateManager {
    /// Create a new in-memory state manager
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }
}

impl Default for MemoryStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateManager for MemoryStateManager {
    async fn set<T: serde::Serialize + Send + Sync + 'static>(
        &self,
        key: &str,
        value: &T,
    ) -> StateManagerResult<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| StateManagerError::StoreError(e.to_string()))?;

        self.data.insert(key.to_string(), json);

        Ok(())
    }

    async fn get<T: serde::de::DeserializeOwned + Send + Sync + 'static>(
        &self,
        key: &str,
    ) -> StateManagerResult<Option<T>> {
        if let Some(json) = self.data.get(key) {
            let value = serde_json::from_str(&json)
                .map_err(|e| StateManagerError::RetrieveError(e.to_string()))?;

            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, key: &str) -> StateManagerResult<()> {
        self.data.remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ports::audio_player::GuildId;
    use crate::core::ports::state_manager::GuildConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn test_set_and_get() {
        let manager = MemoryStateManager::new();
        let value = 42u32;

        manager.set("test_key", &value).await.unwrap();

        let retrieved: Option<u32> = manager.get("test_key").await.unwrap();
        assert_eq!(retrieved, Some(42));
    }

    #[tokio::test]
    async fn test_delete() {
        let manager = MemoryStateManager::new();
        let value = "test value".to_string();

        manager.set("test_key", &value).await.unwrap();
        manager.delete("test_key").await.unwrap();

        let retrieved: Option<String> = manager.get("test_key").await.unwrap();
        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_guild_config() {
        let manager = MemoryStateManager::new();
        let guild_id = GuildId(12345);

        // Get default config
        let config = manager.get_guild_config(guild_id).await.unwrap();
        assert_eq!(config.idle_timeout_minutes, Some(5));

        // Set custom config
        let mut custom_config = GuildConfig::default();
        custom_config.idle_timeout_minutes = Some(10);
        custom_config.default_volume = 0.8;

        manager
            .set_guild_config(guild_id, &custom_config)
            .await
            .unwrap();

        // Get custom config
        let retrieved = manager.get_guild_config(guild_id).await.unwrap();
        assert_eq!(retrieved.idle_timeout_minutes, Some(10));
        assert_eq!(retrieved.default_volume, 0.8);
    }

    #[tokio::test]
    async fn test_idle_timeout() {
        let manager = MemoryStateManager::new();
        let guild_id = GuildId(12345);

        // Set timeout to 2 minutes
        manager
            .set_idle_timeout(guild_id, Some(Duration::from_secs(120)))
            .await
            .unwrap();

        // Get timeout
        let timeout = manager.get_idle_timeout(guild_id).await.unwrap();
        assert_eq!(timeout, Some(Duration::from_secs(120)));

        // Disable timeout
        manager.set_idle_timeout(guild_id, None).await.unwrap();

        // Check timeout is disabled
        let timeout = manager.get_idle_timeout(guild_id).await.unwrap();
        assert_eq!(timeout, None);
    }
}
