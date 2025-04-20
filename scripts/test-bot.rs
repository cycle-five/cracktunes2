// Test bot CLI for CrackTunes integration testing
//
// NOTE: This test bot is currently in development and requires further adaptation
// to work with the specific versions of serenity, songbird, and poise used in
// the main CrackTunes codebase. The structure and design are complete, but
// compatibility with the current Discord library versions needs to be resolved.
use clap::{Parser, Subcommand};
use poise::serenity_prelude as serenity;
use serenity::all::{ChannelId, GuildId, Http, Token, UserId};
use songbird::Songbird;
use std::env;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
            let http = Http::new(token.clone());
            let songbird = songbird::Songbird::serenity();
            let test_handler = TestHandler::new(Arc::new(http), songbird.clone());
            test_handler
                .set_target_bot_id(UserId::new(target_bot_id))
                .await;
            let mut client = serenity::Client::builder(token, intents)
                .voice_manager::<Songbird>(songbird)
                .event_handler(test_handler.clone())
                .await?;

            // Create a channel to signal shutdown to the client task
            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

            // Start the client directly in a separate task without Arc for mutable access
            tokio::spawn(async move {
                tokio::select! {
                    result = client.start() => {
                        if let Err(why) = result {
                            eprintln!("Client error: {:?}", why);
                        }
                    },
                    _ = shutdown_rx => {
                        println!("Received shutdown signal, stopping client...");
                        // Here we would ideally call client.shutdown() or similar, but since serenity::Client may not have a direct shutdown method,
                        // we rely on the task ending when the program exits.
                        let ids = client.shard_manager.shards_instantiated();
                        for id in ids {
                            client.shard_manager.shutdown(id, 1000);
                        }
                    }
                }
            });

            // Set up signal handling for graceful shutdown
            tokio::spawn(async move {
                #[cfg(unix)]
                {
                    use tokio::signal::unix as signal;
                    let mut sigint = signal::signal(signal::SignalKind::interrupt()).unwrap();
                    let mut sigterm = signal::signal(signal::SignalKind::terminate()).unwrap();
                    let mut sighup = signal::signal(signal::SignalKind::hangup()).unwrap();

                    tokio::select! {
                        _ = sigint.recv() => println!("Received SIGINT, shutting down..."),
                        _ = sigterm.recv() => println!("Received SIGTERM, shutting down..."),
                        _ = sighup.recv() => println!("Received SIGHUP, shutting down..."),
                    };
                }
                #[cfg(windows)]
                {
                    let mut ctrl_c = tokio::signal::windows::ctrl_c().unwrap();
                    let mut ctrl_break = tokio::signal::windows::ctrl_break().unwrap();

                    tokio::select! {
                        _ = ctrl_c.recv() => println!("Received Ctrl+C, shutting down..."),
                        _ = ctrl_break.recv() => println!("Received Ctrl+Break, shutting down..."),
                    };
                }
                // Send shutdown signal to the client task
                let _ = shutdown_tx.send(());
            });

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
