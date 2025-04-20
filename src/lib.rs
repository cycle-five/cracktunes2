#![feature(iter_chain)]
pub mod event_handlers;
pub use event_handlers::*;
pub mod logging;
pub use logging::*;
pub mod connection;
pub use connection::*;
pub mod commands;
pub use commands::*;
pub mod guild_cache;
pub use guild_cache::*;

// Make test module available for integration testing
#[cfg(any(test, feature = "test-bot"))]
pub mod test;

// Define the context type for poise
pub type Context<'a> = poise::Context<'a, Data, crack_types::Error>;

//------------------------------------
// crack_types imports
//------------------------------------
use crack_types::{Error, QueryType};

//------------------------------------
// External library imports
//------------------------------------
use rusty_ytdl::RequestOptions;
use rusty_ytdl::{search, search::YouTube};
use serenity::all::GuildId;
use songbird::input::AuxMetadata;
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
pub const EMPTY_QUEUE: &str = "Queue is empty.";
pub const NOTHING_PLAYING: &str = "Nothing is playing.";
pub const NOTHING_PLAYING_PAUSE: &str = "Nothing is playing to pause.";
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

// static CRACK_TRACK_CLIENT: LazyLock<CrackTrackClient> = LazyLock::new(|| {
//     println!("{CREATING}: CrackTrackClient...");
//     CrackTrackClient::new_with_clients(REQ_CLIENT.clone(), YOUTUBE_CLIENT.clone())
// });

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

impl TrackMetadata {
    #[must_use]
    /// Get the track duration in seconds.
    /// Returns 0 if the duration is not available.
    pub fn get_duration_as_secs(&self) -> u64 {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.duration.map(|d| d.as_secs()))
            .unwrap_or(0)
    }
}

/// Our user data structure, all commands have access to it,
/// Used for resolving tracks and managing queues. Also holds other clients like
/// reqwest, `rusty_ytdl`, and songbird.
#[derive(Clone)]
pub struct CrackData {
    pub req_client: reqwest::Client,
    pub yt_client: rusty_ytdl::search::YouTube,
    pub guild_cache_map: dashmap::DashMap<GuildId, GuildCache>,
    // Songbird instance for audio
    pub songbird: Arc<songbird::Songbird>,
}

impl fmt::Debug for CrackData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrackData")
            .field("req_client", &"reqwest::Client")
            .field("yt_client", &"rusty_ytdl::search::YouTube")
            .field("guild_cache_map", &"DashMap<GuildId, GuildCache>")
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
        // don't need to do anything here right now
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
        f.debug_struct("Data").field("0", &self.0).finish()
    }
}

impl Data {
    /// Bump the activity timestamp for a guild.
    pub fn bump_activity(&self, guild_id: GuildId) {
        if let Some(cache) = self.guild_cache_map.get(&guild_id) {
            cache.idle_timeout.bump_activity();
        } else {
            error!("No guild cache found for guild_id: {guild_id:?}");
        }
    }
}

/// Struct to hold search suggestions
/// for a given query. This is used to display search results
#[derive(Debug, Clone)]
pub struct SearchSuggestion {
    pub title: String,
    pub url: String,
    pub duration: String,
}

impl fmt::Display for SearchSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Limit the title to 20 characters for display
        let title = &self.title[..20];
        write!(f, "[{}]({}) - {}", title, self.url, self.duration)
    }
}

/// Get a suggestion from a query. Use the global static client.
/// # Errors
/// Returns an error if the query fails.
pub async fn suggestion2(query: &str) -> Vec<SearchSuggestion> {
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
                    search::SearchResult::Video(video) => Some(SearchSuggestion {
                        title: video.title,
                        url: video.url,
                        duration: video.duration.to_string(),
                    }),
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
