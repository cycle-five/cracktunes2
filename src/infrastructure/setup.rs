use serenity::http::Http;
use std::sync::Arc;

use crate::{
    adapters::{
        discord::{
            serenity_message_handler::SerenityMessageHandler, songbird_player::SongbirdPlayer,
        },
        storage::memory_state_manager::MemoryStateManager,
        youtube::rusty_ytdl_provider::RustyYtdlProvider,
    },
    core::services::{connection_service::ConnectionService, music_service::MusicService},
};

/// App services container - holds all the core services
#[derive(Clone, Debug)]
pub struct AppServices {
    pub music_service: Arc<
        MusicService<RustyYtdlProvider, SongbirdPlayer, SerenityMessageHandler, MemoryStateManager>,
    >,
    pub connection_service: Arc<ConnectionService<SongbirdPlayer, MemoryStateManager>>,
}

/// Initialize all adapters and services
pub async fn initialize_services(
    http: Arc<Http>,
    songbird: Arc<songbird::Songbird>,
) -> Result<AppServices, Box<dyn std::error::Error + Send + Sync>> {
    // Initialize adapters
    let reqwest_client = reqwest::Client::new();
    let audio_provider = Arc::new(RustyYtdlProvider::new(reqwest_client)?);
    let audio_player = Arc::new(SongbirdPlayer::new(songbird));
    let message_handler = Arc::new(SerenityMessageHandler::new(http));
    let state_manager = Arc::new(MemoryStateManager::new());

    // Initialize services
    let music_service = Arc::new(MusicService::new(
        audio_provider.clone(),
        audio_player.clone(),
        message_handler.clone(),
        state_manager.clone(),
    ));

    let connection_service = Arc::new(ConnectionService::new(
        audio_player.clone(),
        state_manager.clone(),
    ));

    Ok(AppServices {
        music_service,
        connection_service,
    })
}

/// App data container for poise framework - contains core services and other framework data
#[derive(Debug)]
pub struct Data {
    pub services: AppServices,
    pub start_time: std::time::Instant,
}

impl Data {
    pub fn new(services: AppServices) -> Self {
        Self {
            services,
            start_time: std::time::Instant::now(),
        }
    }
}
