use self::serenity::model::{channel::Message, id::UserId};
use poise::serenity_prelude as serenity;
use std::{collections::HashSet, sync::Arc};
use std::{fmt, sync::atomic::AtomicUsize};
use tokio::sync::RwLock;

type QueueMessage = (Message, Arc<RwLock<usize>>);

#[derive(Debug, Clone)]
pub struct GuildCache {
    pub autoplay: bool,
    pub queue_messages: Vec<QueueMessage>,
    pub idle_timeout: IdleTimeoutInfo,
    pub current_skip_votes: HashSet<UserId>,
}

impl Default for GuildCache {
    fn default() -> Self {
        Self {
            autoplay: true,
            queue_messages: Vec::new(),
            idle_timeout: IdleTimeoutInfo::default(),
            current_skip_votes: HashSet::new(),
        }
    }
}

impl GuildCache {
    /// Add a skip vote for a user
    pub fn add_skip_vote(&mut self, user_id: UserId) {
        self.current_skip_votes.insert(user_id);
    }

    /// Remove a skip vote for a user
    pub fn remove_skip_vote(&mut self, user_id: UserId) {
        self.current_skip_votes.remove(&user_id);
    }

    /// Build a `GuildCache` with the specified idle timeout
    #[must_use]
    pub fn with_idle_timeout(&mut self, timeout_minutes: usize) -> Self {
        Self {
            idle_timeout: IdleTimeoutInfo {
                timeout_minutes: Arc::new(AtomicUsize::new(timeout_minutes)),
                last_activity: Arc::new(AtomicUsize::new(0)),
            },
            ..self.clone()
        }
    }
}

#[derive(Default, Debug)]
pub struct GuildCacheMap;

/// Struct to hold idle timeout information for a guild
#[derive(Clone)]
pub struct IdleTimeoutInfo {
    pub timeout_minutes: Arc<AtomicUsize>, // 0 means never leave
    pub last_activity: Arc<AtomicUsize>,   // Timestamp in minutes since joining
}

impl Default for IdleTimeoutInfo {
    fn default() -> Self {
        Self {
            timeout_minutes: Arc::new(AtomicUsize::new(5)), // Default to 5 minutes
            last_activity: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl IdleTimeoutInfo {
    /// Increment the `last_activity` timestamp by 1 (for marking active actions)
    pub fn bump_activity(&self) {
        let current_time = self
            .last_activity
            .load(std::sync::atomic::Ordering::Relaxed);
        self.last_activity
            .store(current_time + 1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set the `last_activity` timestamp to a specific value (for syncing with time tracking)
    pub fn set_activity_to(&self, time: usize) {
        self.last_activity
            .store(time, std::sync::atomic::Ordering::Relaxed);
    }
}

impl fmt::Debug for IdleTimeoutInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdleTimeoutInfo")
            .field("timeout_minutes", &self.timeout_minutes)
            .field("last_activity", &self.last_activity)
            .finish()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[tokio::test]
    async fn test_guild_cache() {
        let guild_cache = GuildCache::default();
        assert!(guild_cache.autoplay);
        assert_eq!(guild_cache.queue_messages.len(), 0);
        assert_eq!(guild_cache.current_skip_votes.len(), 0);
    }

    // Test inserting queue messages and getting them out
    #[tokio::test]
    async fn test_queue_messages() {
        let guild_cache = GuildCache::default();
        let message = Message::default();
        let queue_message = (message, Arc::new(RwLock::new(0)));
        let mut guild_cache = guild_cache.clone();
        guild_cache.queue_messages.push(queue_message.clone());
        assert_eq!(guild_cache.queue_messages.len(), 1);
    }
}
