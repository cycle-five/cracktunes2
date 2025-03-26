use crate::Context;
use poise::serenity_prelude as serenity;
use serenity::small_fixed_array::FixedString;
use std::path::Path;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Log directory name
pub const LOG_DIR: &str = "logs";
/// Command log file name
pub const COMMAND_LOG_FILE: &str = "commands";
/// Balance log file name
pub const BALANCE_LOG_FILE: &str = "tracks";

/// Initialize the logging system with console and file outputs
pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create log directory if it doesn't exist
    if !Path::new(LOG_DIR).exists() {
        std::fs::create_dir_all(LOG_DIR)?;
    }

    // Set up file appenders with daily rotation
    let command_file = RollingFileAppender::new(Rotation::DAILY, LOG_DIR, COMMAND_LOG_FILE);
    let balance_file = RollingFileAppender::new(Rotation::DAILY, LOG_DIR, BALANCE_LOG_FILE);

    // Create a layer for console output (human-readable format)
    let console_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_ansi(true);

    // Create a layer for command logs (JSON format)
    let command_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_ansi(false)
        .json()
        .with_writer(command_file);

    // Create a layer for balance logs (JSON format)
    let balance_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_ansi(false)
        .json()
        .with_writer(balance_file);

    // Set up the subscriber with all layers
    // Use env filter to allow runtime configuration of log levels
    // Default to INFO level if not specified, but filter out serenity heartbeat logs
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info")
            // Filter out serenity logs
            .add_directive("serenity=error".parse().unwrap())
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(command_layer)
        .with(balance_layer)
        .init();

    info!("Logging system initialized");
    Ok(())
}

/// Log a command execution - thread-safe version
pub fn log_command(
    command_name: &str,
    guild_id: Option<u64>,
    user_id: u64,
    args: &str,
    success: bool,
) {
    // Clone values to avoid thread safety issues
    let command_name = command_name.to_owned();
    let guild_id_str = guild_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "DM".to_string());
    let user_id_str = user_id.to_string();
    let args = args.to_owned();

    // Use tokio spawn to avoid thread safety issues
    tokio::spawn(async move {
        if success {
            info!(
                target: "cracktunes::command",
                command = command_name,
                guild_id = guild_id_str,
                user_id = user_id_str,
                arguments = args,
                result = "success",
                "Command executed successfully"
            );
        } else {
            info!(
                target: "cracktunes::command",
                command = command_name,
                guild_id = guild_id_str,
                user_id = user_id_str,
                arguments = args,
                result = "failure",
                "Command execution failed"
            );
        }
    });
}

/// Log a track play event - thread safe version using IDs
pub async fn log_track_play_ids(
    ctx: Context<'_>,
    guild_id: serenity::all::GuildId,
    user_id: serenity::all::UserId,
    channel_id: serenity::all::ChannelId,
    track_title: &str,
) {
    // Clone these values to avoid thread safety issues
    let guild_id_str = guild_id.to_string();
    let user_id_str = user_id.to_string();
    let channel_id_str = channel_id.to_string();
    let track_title_owned = track_title.to_owned();

    // Use tokio spawn to avoid thread safety issues with context
    tokio::spawn(async move {
        info!(
            target: "cracktunes::track",
            guild_id = guild_id_str,
            user_id = user_id_str,
            channel_id = channel_id_str,
            track_title = track_title_owned,
            "Track played"
        );
    });
}

/// Thread-safe version of track play logger
pub fn log_track_play_names(
    guild_name: &str,
    user_name: &str,
    channel_name: &str,
    track_title: &str,
) {
    // Clone values to avoid thread safety issues
    let guild_name = guild_name.to_owned();
    let user_name = user_name.to_owned();
    let channel_name = channel_name.to_owned();
    let track_title = track_title.to_owned();

    tokio::spawn(async move {
        info!(
            target: "cracktunes::track",
            guild_name,
            user_name,
            channel_name,
            track_title,
            "Track played"
        );
    });
}
