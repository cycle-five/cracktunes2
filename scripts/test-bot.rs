// Test bot CLI for CrackTunes integration testing
//
// NOTE: This test bot is currently in development and requires further adaptation
// to work with the specific versions of serenity, songbird, and poise used in
// the main CrackTunes codebase. The structure and design are complete, but
// compatibility with the current Discord library versions needs to be resolved.
use clap::{Parser, Subcommand};
use serenity::all::{ChannelId, GuildId, Http, Token, UserId};
use std::env;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    prelude::*,
    util::SubscriberInitExt,
    EnvFilter,
};

use cracktunes::test::{run_test_bot, run_test_scenario, TestHandler};

/// CrackTunes Test Bot CLI
#[derive(Parser, Debug)]
#[command(author, version, about = "Test bot for CrackTunes integration testing", long_about = None)]
struct Args {
    /// Run specific test scenarios
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run specific test scenarios
    RunTests {
        /// Guild ID to run tests in
        #[arg(long)]
        guild: u64,

        /// Text channel ID to send commands to
        #[arg(long)]
        channel: u64,

        /// Voice channel ID to join for voice tests
        #[arg(long)]
        voice: u64,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Environment variables needed:
    // TEST_BOT_TOKEN - Discord token for the test bot
    // TARGET_BOT_ID - User ID of the CrackTunes bot to test

    let args = Args::parse();

    info!("Starting CrackTunes Test Bot");
    info!("Arguments: {:?}", args);
    info!("Environment variables: {:?}", env::vars());

    match args.command {
        Some(Commands::RunTests {
            guild,
            channel,
            voice,
        }) => {
            // Convert the IDs to serenity types
            let guild_id = GuildId::new(guild);
            let channel_id = ChannelId::new(channel);
            let voice_id = ChannelId::new(voice);

            // Set up the test bot and run the specified test scenario
            let token = Token::from_env("TEST_BOT_TOKEN")?;
            let target_bot_id = env::var("TARGET_BOT_ID")?.parse::<u64>()?;

            // Create the test handler
            let http = Http::new(token);
            let songbird = songbird::Songbird::serenity();
            let test_handler = TestHandler::new(Arc::new(http), songbird.clone());
            test_handler
                .set_target_bot_id(UserId::new(target_bot_id))
                .await;

            // Run the test scenario
            match run_test_scenario(Arc::new(test_handler), channel_id, voice_id, guild_id).await {
                Ok(_) => println!("Test scenario completed successfully!"),
                Err(e) => eprintln!("Test scenario failed: {}", e),
            }
        }
        None => {
            // Run the test bot in interactive mode
            run_test_bot().await?;
        }
    }

    Ok(())
}
