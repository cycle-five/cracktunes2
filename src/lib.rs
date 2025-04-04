#![feature(iter_chain)]

// Original architecture - will be gradually phased out
pub mod event_handlers;
pub use event_handlers::*;
pub mod logging;
pub use logging::*;
pub mod connection;
pub use connection::*;
pub mod commands;
pub use commands::*;

// New hexagonal architecture
pub mod adapters;
pub mod core;
pub mod infrastructure;

#[cfg(test)]
pub mod test;

// Define the context type for poise (legacy)
pub type Context<'a> = poise::Context<'a, Data, crack_types::Error>;
pub type Error = crack_types::Error;

//------------------------------------
// crack_types imports
//------------------------------------
// use crack_osint::ipqs::IpqsClient;
use crack_types::http::parse_url;
use crack_types::QueryType;
//------------------------------------
// External library imports
//------------------------------------
use clap::{Parser, Subcommand};
use rusty_ytdl::RequestOptions;
use rusty_ytdl::{search, search::YouTube};
use songbird::input::AuxMetadata;
use std::sync::atomic::AtomicUsize;
use std::sync::LazyLock;
use tracing::{error, info};
//------------------------------------
// Standard library imports
//------------------------------------
use std::fmt::{self, Debug};
use std::sync::Arc;

//------------------------------------
// Constants
//------------------------------------
pub const CREATING: &str = "Creating";
pub const DEFAULT_PLAYLIST_LIMIT: u64 = 50;
pub const EMPTY_QUEUE: &str = "Queue is empty or display not built.";
pub const NEW_FAILED: &str = "New failed";
pub const REQ_CLIENT_STR: &str = "Reqwest client";
pub const UNKNOWN_TITLE: &str = "Unknown title";
pub const UNKNOWN_URL: &str = "";
pub const UNKNOWN_DURATION: &str = "??:??:??";
pub const YOUTUBE_CLIENT_STR: &str = "YouTube client";

//------------------------------------
// Module statics.
// I did this so that I could easily make sure only one instance of the client is created
// and that it's available to all functions in the module.
// I've read elsewhere that this is a bit of a bad practice, and that it's better to put
// the clients in a context struct and pass it around everywhere. Other than the potential
// problems from it getting out of hand if the module is too big, I don't see a problem with it.
//------------------------------------
static REQ_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    println!("{CREATING}: {REQ_CLIENT_STR}...");
    build_configured_reqwest_client()
});

static YOUTUBE_CLIENT: LazyLock<rusty_ytdl::search::YouTube> = LazyLock::new(|| {
    println!("{CREATING}: {YOUTUBE_CLIENT_STR}...");
    let req_client = REQ_CLIENT.clone();
    let opts = RequestOptions {
        client: Some(req_client.clone()),
        ..Default::default()
    };
    rusty_ytdl::search::YouTube::new_with_options(&opts)
        .unwrap_or_else(|_| panic!("{NEW_FAILED} {YOUTUBE_CLIENT_STR}"))
});

/// Build a configured reqwest client for use in the `CrackTrackClient`.
///
/// # Panics
/// Panics if the reqwest client cannot be built.
#[must_use]
pub fn build_configured_reqwest_client() -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .use_rustls_tls()
        .cookie_store(true)
        .build()
        .unwrap_or_else(|_| panic!("{NEW_FAILED} {REQ_CLIENT_STR}"))
}

pub fn get_youtube_client() -> YouTube {
    YOUTUBE_CLIENT.clone()
}

pub fn get_reqwest_client() -> reqwest::Client {
    REQ_CLIENT.clone()
}

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

/// Struct to hold the metadata we additionally want to track for each track.
#[derive(Clone)]
pub struct TrackMetadata {
    pub requesting_user: String,
    pub requesting_user_id: String,
    pub metadata: Option<AuxMetadata>,
}

unsafe impl Send for TrackMetadata {}
unsafe impl Sync for TrackMetadata {}

impl Debug for TrackMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrackMetadata")
            .field("requesting_user", &self.requesting_user)
            .field("requesting_user_id", &self.requesting_user_id)
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// Client for resolving tracks and managing queues. Also holds other clients like
/// reqwest, `rusty_ytdl`, and songbird.
#[derive(Clone)]
pub struct CrackData {
    pub req_client: reqwest::Client,
    pub yt_client: rusty_ytdl::search::YouTube,
    // Map of guild IDs to idle timeout information
    pub idle_timeouts: dashmap::DashMap<serenity::all::GuildId, IdleTimeoutInfo>,
    // Songbird instance for audio
    pub songbird: Arc<songbird::Songbird>,
}

impl fmt::Debug for CrackData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrackTrackClient")
            .field("req_client", &"reqwest::Client")
            .field("yt_client", &"rusty_ytdl::search::YouTube")
            .field("idle_timeouts", &self.idle_timeouts)
            .field("songbird", &"Arc<songbird::Songbird>")
            .finish()
    }
}

///
/// The data structure that will be available in all command contexts.
/// This is a thin wrapper around [`CrackData`].
///
//#[derive(Clone)]
pub struct Data(pub CrackData);

impl Drop for Data {
    fn drop(&mut self) {
        // Clean up resources if needed
    }
}

impl std::ops::Deref for Data {
    type Target = CrackData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Data {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Data").field("client", &self.0).finish()
    }
}

#[derive(Debug, Clone)]
pub struct SearchSuggestion {
    pub title: String,
    pub url: String,
    pub duration: String,
}

impl fmt::Display for SearchSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]({}) - ({})", self.title, self.url, self.duration)
    }
}

/// Get a suggestion from a query. Use the global static client.
/// # Errors
/// Returns an error if the query fails.
pub async fn suggestion2(query: &str) -> Vec<SearchSuggestion> {
    // Access the static directly instead of cloning it
    if query.len() < 3 {
        return Vec::new();
    }
    let client = YOUTUBE_CLIENT.clone();
    let search_options = search::SearchOptions {
        limit: 5,
        ..Default::default()
    };
    let res = client.search(query, Some(&search_options)).await;
    match res {
        Ok(results) => {
            let suggestions: Vec<SearchSuggestion> = results
                .into_iter()
                .filter_map(|x| match x {
                    search::SearchResult::Video(video) => {
                        //Some(format!("{} * {}", video.title, video.duration))
                        Some(SearchSuggestion {
                            title: video.title,
                            url: video.url,
                            duration: video.duration.to_string(),
                        })
                    }
                    _ => None,
                })
                .collect();
            info!("Suggestions: {suggestions:?}");
            suggestions
        }
        Err(e) => {
            error!("Error getting suggestions: {e:?}");
            Vec::new()
        }
    }
}

/// Get a suggestion from a query. Use the global static client.
/// # Errors
/// Returns an error if the query fails.
pub async fn suggestion(query: &str) -> Result<Vec<String>, Error> {
    if query.len() < 3 {
        return Ok(Vec::new());
    }
    let client = YOUTUBE_CLIENT.clone();
    suggestion_yt(client, query).await
}

/// Get a suggestion from a query. Passthrough to [`rusty_ytdl::search::YouTube::suggestion`].
/// # Errors
/// Returns an error if the query fails.
pub async fn suggestion_yt(client: YouTube, query: &str) -> Result<Vec<String>, Error> {
    let query = query.replace('"', "");
    if query.is_empty() {
        return Ok(Vec::new());
    }
    client
        .suggestion(query, Some(search::LanguageTags::EN))
        .await
        .map_err(Into::into)
        .map(|res| res.into_iter().map(|x| x.replace('"', "")).collect())
}

pub fn check_msg(result: serenity::Result<serenity::all::Message>) {
    if let Err(why) = result {
        error!("Error sending message: {why:?}");
    }
}

/// Args struct for the CLI.
#[derive(Parser, Debug)]
#[command(
    version = "1.0",
    author = "Cycle Five <cycle.five@proton.me>",
    about = "A simple CLI harness for testing new modules for Crack Tunes."
)]
struct Cli {
    /// The command to run
    #[command(subcommand)]
    command: Commands,
}

/// The command to run.
#[derive(Subcommand, Debug)]
enum Commands {
    Suggest {
        /// The query to get suggestions for.
        query: String,
    },
    Ipqs {
        ip: String,
    },
    Resolve {
        /// URL of the video / playlist to resolve.
        #[arg(value_parser = parse_url)]
        url: url::Url,
    },
    Query {
        /// The query to resolve.
        query: String,
    },
}

/// Get the query type from a youtube URL. Video or playlist.
fn _yt_url_type(url: &url::Url) -> QueryType {
    if url.path().contains("playlist")
        || url.query_pairs().any(|(k, _)| k == "list") && url.path().contains("watch")
    {
        QueryType::PlaylistLink(url.to_string())
    } else {
        QueryType::VideoLink(url.to_string())
    }
}
