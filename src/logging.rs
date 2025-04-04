use crate::core::models::error::AppError;
use crate::{Data, infrastructure::setup::Data as V2Data};
use poise::serenity_prelude as serenity;
use poise::{Context, FrameworkError};
use std::path::Path;
use std::time::Instant;
use tracing::{error, info};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    prelude::*,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Log directory name
pub const LOG_DIR: &str = "logs";
/// Command log file name
pub const COMMAND_LOG_FILE: &str = "commands";
/// Track log file name
pub const TRACK_LOG_FILE: &str = "tracks";
/// Error log file name
pub const ERROR_LOG_FILE: &str = "errors";

/// Initialize the logging system with console and file outputs
/// # Errors
/// Returns an error if the log directory cannot be created or if the file appender fails to initialize.
/// # Panics
/// Panics if the directive parsing fails (never happens).
pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create log directory if it doesn't exist
    if !Path::new(LOG_DIR).exists() {
        std::fs::create_dir_all(LOG_DIR)?;
    }

    // Set up file appenders with daily rotation
    let command_file = RollingFileAppender::new(Rotation::DAILY, LOG_DIR, COMMAND_LOG_FILE);
    let track_file = RollingFileAppender::new(Rotation::DAILY, LOG_DIR, TRACK_LOG_FILE);
    let error_file = RollingFileAppender::new(Rotation::DAILY, LOG_DIR, ERROR_LOG_FILE);

    // Create a layer for console output (human-readable format)
    let console_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true);
    // .with_filter(FilterFn::new(|metadata| {
    //     metadata.target().starts_with("cracktunes")
    // }));

    // Create a layer for command logs (JSON format)
    let command_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_ansi(false)
        .json()
        .with_writer(command_file)
        .with_filter(FilterFn::new(|metadata| {
            metadata.target() == "cracktunes::command"
        }));

    // Create a layer for track logs (JSON format)
    let track_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_ansi(false)
        .json()
        .with_writer(track_file)
        .with_filter(FilterFn::new(|metadata| {
            metadata.target() == "cracktunes::track"
        }));

    // Create a layer for error logs (JSON format)
    let error_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_ansi(false)
        .json()
        .with_writer(error_file)
        .with_filter(FilterFn::new(|metadata| {
            metadata.target().starts_with("cracktunes")
                && metadata.level() >= &tracing::Level::ERROR
        }));

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
        .with(track_layer)
        .with(error_layer)
        .init();

    info!("Logging system initialized");
    Ok(())
}

// Store command start time in the context data
thread_local! {
    static COMMAND_START_TIME: std::cell::RefCell<Option<Instant>> = const { std::cell::RefCell::new(None) };
}

/// Core logging function for command start
fn log_command_start_core(
    command_name: &str,
    guild_id: &str,
    user_id: &str,
    args: &str,
) {
    // Store the start time for later use in post_command
    COMMAND_START_TIME.with(|cell| {
        *cell.borrow_mut() = Some(Instant::now());
    });

    info!(
        target: "cracktunes::command",
        command = %command_name,
        guild_id = %guild_id,
        user_id = %user_id,
        arguments = %args,
        event = "start",
        "Command execution started"
    );
}

/// Core logging function for command end
fn log_command_end_core(
    command_name: &str,
    guild_id: &str,
    user_id: &str,
) {
    // Calculate execution time
    let duration =
        COMMAND_START_TIME.with(|cell| cell.borrow_mut().take().map(|start| start.elapsed()));

    let duration_ms = u64::try_from(duration.map_or(0, |d| d.as_millis())).unwrap_or_default();
    info!(
        target: "cracktunes::command",
        command = %command_name,
        guild_id = %guild_id,
        user_id = %user_id,
        duration_ms = duration_ms,
        event = "end",
        "Command execution completed"
    );
}

/// Core logging function for command errors
fn log_command_error_core(
    command_name: &str,
    guild_id: &str,
    user_id: &str,
    error_msg: &str,
    error_type: &str,
) {
    error!(
        target: "cracktunes::error",
        command = %command_name,
        guild_id = %guild_id,
        user_id = %user_id,
        error = %error_msg,
        error_type = %error_type,
        "Command error"
    );
}

/// Core logging function for command check failures
fn log_command_check_failed_core(
    command_name: &str,
    guild_id: &str,
    user_id: &str,
    error_msg: &str,
) {
    error!(
        target: "cracktunes::error",
        command = %command_name,
        guild_id = %guild_id,
        user_id = %user_id,
        error = %error_msg,
        "Command check failed"
    );
}

/// Core logging function for other framework errors
fn log_other_framework_error_core(error_type: &str, error: &str) {
    error!(
        target: "cracktunes::error",
        error_type = %error_type,
        error = %error,
        "Other framework error"
    );
}

// ----- Original architecture logging functions -----

/// Log the start of a command execution (pre-command hook) - Original architecture
pub fn log_command_start(ctx: Context<'_, Data, crack_types::Error>) {
    let command_name = ctx.command().qualified_name.clone();
    let guild_id = ctx
        .guild_id()
        .map_or_else(|| "DM".to_string(), |id| id.get().to_string());
    let user_id = ctx.author().id.get().to_string();

    // Attempt to format arguments
    let args = if ctx.command().parameters.is_empty() {
        String::new()
    } else {
        format!("{:?}", ctx.invocation_string())
    };

    log_command_start_core(&command_name, &guild_id, &user_id, &args);
}

/// Log the end of a command execution (post-command hook) - Original architecture
pub fn log_command_end(ctx: Context<'_, Data, crack_types::Error>) {
    let command_name = ctx.command().qualified_name.clone();
    let guild_id = ctx
        .guild_id()
        .map_or_else(|| "DM".to_string(), |id| id.get().to_string());
    let user_id = ctx.author().id.get().to_string();

    log_command_end_core(&command_name, &guild_id, &user_id);
}

/// Log errors that occur during command execution - Original architecture
pub fn log_command_error(error: &FrameworkError<'_, Data, crack_types::Error>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let command_name = ctx.command().qualified_name.clone();
            let guild_id = ctx
                .guild_id()
                .as_ref()
                .map_or_else(|| "DM".to_string(), ToString::to_string);
            let user_id = ctx.author().id.get().to_string();

            log_command_error_core(
                &command_name, 
                &guild_id, 
                &user_id, 
                &error.to_string(),
                "CommandError",
            );
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            let command_name = ctx.command().qualified_name.clone();
            let guild_id = ctx
                .guild_id()
                .as_ref()
                .map_or_else(|| "DM".to_string(), ToString::to_string);
            let user_id = ctx.author().id.get().to_string();

            let error_msg = error
                .as_ref()
                .map_or_else(|| "Check failed".to_string(), ToString::to_string);

            log_command_check_failed_core(
                &command_name,
                &guild_id,
                &user_id,
                &error_msg,
            );
        }
        err => {
            log_other_framework_error_core(
                std::any::type_name::<FrameworkError<'_, Data, crack_types::Error>>(),
                &format!("{err:?}"),
            );
        }
    }
}

// ----- New architecture logging functions -----

/// Log the start of a command execution (pre-command hook) - New architecture
pub fn log_command_start_v2(ctx: Context<'_, V2Data, AppError>) {
    let command_name = ctx.command().qualified_name.clone();
    let guild_id = ctx
        .guild_id()
        .map_or_else(|| "DM".to_string(), |id| id.get().to_string());
    let user_id = ctx.author().id.get().to_string();

    // Attempt to format arguments
    let args = if ctx.command().parameters.is_empty() {
        String::new()
    } else {
        format!("{:?}", ctx.invocation_string())
    };

    log_command_start_core(&command_name, &guild_id, &user_id, &args);
}

/// Log the end of a command execution (post-command hook) - New architecture
pub fn log_command_end_v2(ctx: Context<'_, V2Data, AppError>) {
    let command_name = ctx.command().qualified_name.clone();
    let guild_id = ctx
        .guild_id()
        .map_or_else(|| "DM".to_string(), |id| id.get().to_string());
    let user_id = ctx.author().id.get().to_string();

    log_command_end_core(&command_name, &guild_id, &user_id);
}

/// Log errors that occur during command execution - New architecture
pub fn log_command_error_v2(error: &FrameworkError<'_, V2Data, AppError>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let command_name = ctx.command().qualified_name.clone();
            let guild_id = ctx
                .guild_id()
                .as_ref()
                .map_or_else(|| "DM".to_string(), ToString::to_string);
            let user_id = ctx.author().id.get().to_string();

            log_command_error_core(
                &command_name, 
                &guild_id, 
                &user_id, 
                &error.to_string(),
                "AppError",
            );
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            let command_name = ctx.command().qualified_name.clone();
            let guild_id = ctx
                .guild_id()
                .as_ref()
                .map_or_else(|| "DM".to_string(), ToString::to_string);
            let user_id = ctx.author().id.get().to_string();

            let error_msg = error
                .as_ref()
                .map_or_else(|| "Check failed".to_string(), ToString::to_string);

            log_command_check_failed_core(
                &command_name,
                &guild_id,
                &user_id,
                &error_msg,
            );
        }
        err => {
            log_other_framework_error_core(
                std::any::type_name::<FrameworkError<'_, V2Data, AppError>>(),
                &format!("{err:?}"),
            );
        }
    }
}

/// Log a track play event
pub fn log_track_play(
    guild_id: serenity::all::GuildId,
    user_id: serenity::all::UserId,
    channel_id: serenity::all::ChannelId,
    track_title: &str,
) {
    // Convert IDs to strings to avoid thread-safety issues
    let guild_id_str = guild_id.to_string();
    let user_id_str = user_id.to_string();
    let channel_id_str = channel_id.to_string();
    let track_title = track_title.to_owned();

    info!(
        target: "cracktunes::track",
        guild_id = %guild_id_str,
        user_id = %user_id_str,
        channel_id = %channel_id_str,
        track_title = %track_title,
        "Track played"
    );
}
