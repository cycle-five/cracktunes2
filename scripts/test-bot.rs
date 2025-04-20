// Test bot CLI for CrackTunes integration testing
//
// NOTE: This test bot is currently in development and requires further adaptation
// to work with the specific versions of serenity, songbird, and poise used in
// the main CrackTunes codebase. The structure and design are complete, but
// compatibility with the current Discord library versions needs to be resolved.
use clap::{Parser, Subcommand};
use poise::serenity_prelude as serenity;
use serenity::all::{ChannelId, GuildId, Http, Token, UserId};
use std::env;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{
    layer::SubscriberExt,
    //prelude::*,
    util::SubscriberInitExt,
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
            let intents = serenity::GatewayIntents::GUILD_VOICE_STATES
                | serenity::GatewayIntents::GUILD_MESSAGES
                | serenity::GatewayIntents::MESSAGE_CONTENT;
            let target_bot_id = env::var("TARGET_BOT_ID")?.parse::<u64>()?;

            // Create the test handler
            let http = Http::new(token);
            let songbird = songbird::Songbird::serenity();
            let test_handler = TestHandler::new(Arc::new(http), songbird.clone());
            test_handler
                .set_target_bot_id(UserId::new(target_bot_id))
                .await;
            let client = serenity::Client::builder(token, intents)
                .voice_manager::<Songbird>(songbird)
                .event_handler(test_handler.clone())
                .await?;

            match run_client(client).await {
                Ok(_) => println!("Client started successfully!"),
                Err(e) => eprintln!("Failed to start client: {}", e),
            }
            // Run the test scenario
            match run_test_scenario(
                Arc::new(test_handler),
                channel_id.into(),
                voice_id,
                guild_id,
            )
            .await
            {
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

/// Function which runs in a seperate tokio task that starts the client
/// and waits for it to finish.
pub async fn run_client(mut client: serenity::Client) -> Result<(), Box<dyn std::error::Error>> {
    // Start the client and wait for it to finish
    // let data2 = client.data.clone();
    tokio::spawn(async move {
        // Start the client
        #[cfg(unix)]
        {
            use tokio::signal::unix as signal;

            let [mut s1, mut s2, mut s3] = [
                signal::signal(signal::SignalKind::hangup()).unwrap(),
                signal::signal(signal::SignalKind::interrupt()).unwrap(),
                signal::signal(signal::SignalKind::terminate()).unwrap(),
            ];

            tokio::select!(
                v = s1.recv() => v.unwrap(),
                v = s2.recv() => v.unwrap(),
                v = s3.recv() => v.unwrap(),
            );
        }
        #[cfg(windows)]
        {
            let (mut s1, mut s2) = (
                tokio::signal::windows::ctrl_c().unwrap(),
                tokio::signal::windows::ctrl_break().unwrap(),
            );

            tokio::select!(
                v = s1.recv() => v.unwrap(),
                v = s2.recv() => v.unwrap(),
            );
        }
        let ids = client.shard_manager.shards_instantiated();
        for id in ids {
            client.shard_manager.shutdown(id, 1000);
        }
    });

    tokio::spawn(async move {
        if let Err(why) = client.start().await {
            eprintln!("Client error: {:?}", why);
        }
    });

    Ok(())
}
