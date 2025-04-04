use serenity::http::Http;
use std::sync::Arc;

use crate::{
    adapters::{
        discord::{
            serenity_message_handler::SerenityMessageHandler, 
            songbird_player::SongbirdPlayer,
            poise_message_handler::PoiseMessageHandler,
        },
        storage::memory_state_manager::MemoryStateManager,
        youtube::rusty_ytdl_provider::RustyYtdlProvider,
    },
    core::{
        services::{connection_service::ConnectionService, music_service::MusicService},
    },
};

/// App services container - holds all the core services
#[derive(Clone, Debug)]
pub struct AppServices {
    pub music_service: Arc<MusicService<RustyYtdlProvider, SongbirdPlayer, SerenityMessageHandler, MemoryStateManager>>,
    pub poise_music_service: Arc<MusicService<RustyYtdlProvider, SongbirdPlayer, PoiseMessageHandler, MemoryStateManager>>,
    pub connection_service: Arc<ConnectionService<SongbirdPlayer, MemoryStateManager>>,
}

/// Initialize all adapters and services
/// 
/// # Errors
/// 
/// Returns an error if initialization of any adapter or service fails
pub fn initialize_services(
    http: Arc<Http>,
    songbird: Arc<songbird::Songbird>,
) -> Result<AppServices, Box<dyn std::error::Error + Send + Sync>> {
    // Initialize common adapters
    let reqwest_client = reqwest::Client::new();
    let audio_provider = Arc::new(RustyYtdlProvider::new(reqwest_client)?);
    let audio_player = Arc::new(SongbirdPlayer::new(songbird));
    let state_manager = Arc::new(MemoryStateManager::new());
    
    // Create Serenity message handler
    let serenity_handler = Arc::new(SerenityMessageHandler::new(http.clone()));
    
    // Create Poise message handler
    let poise_handler = Arc::new(PoiseMessageHandler::new(http));
    
    // Initialize services with Serenity handler
    let music_service = Arc::new(MusicService::new(
        audio_provider.clone(),
        audio_player.clone(),
        serenity_handler,
        state_manager.clone(),
    ));
    
    // Initialize services with Poise handler
    let poise_music_service = Arc::new(MusicService::new(
        audio_provider,
        audio_player.clone(),
        poise_handler,
        state_manager.clone(),
    ));

    let connection_service = Arc::new(ConnectionService::new(
        audio_player,
        state_manager,
    ));

    Ok(AppServices {
        music_service,
        poise_music_service,
        connection_service,
    })
}

// Global poise message handler to allow access from command hooks
static POISE_HANDLER: std::sync::OnceLock<Arc<PoiseMessageHandler>> = std::sync::OnceLock::new();

/// Get the global `PoiseMessageHandler` instance
#[must_use]
pub fn get_global_poise_handler() -> Option<Arc<PoiseMessageHandler>> {
    POISE_HANDLER.get().cloned()
}

/// Initialize services with Poise message handler - same as regular initialize but returns
/// `poise_music_service` as `music_service` for backward compatibility
/// 
/// # Errors
/// 
/// Returns an error if initialization of any service fails
pub fn initialize_services_with_poise(
    http: Arc<Http>,
    songbird: Arc<songbird::Songbird>,
) -> Result<AppServices, Box<dyn std::error::Error + Send + Sync>> {
    let services = initialize_services(http.clone(), songbird)?;
    
    // Initialize the global handler if not already initialized
    let poise_handler = Arc::new(PoiseMessageHandler::new(http));
    let _ = POISE_HANDLER.set(poise_handler.clone());
    
    // We need to use appropriate types here
    Ok(AppServices {
        music_service: services.music_service,
        poise_music_service: services.poise_music_service,
        connection_service: services.connection_service,
    })
}

/// App data container for poise framework - contains core services and other framework data
#[derive(Debug)]
pub struct Data {
    pub services: AppServices,
    pub start_time: std::time::Instant,
}

impl Data {
    #[must_use]
    pub fn new(services: AppServices) -> Self {
        Self {
            services,
            start_time: std::time::Instant::now(),
        }
    }
}
