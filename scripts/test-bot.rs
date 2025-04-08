// Test bot CLI for CrackTunes integration testing
//
// NOTE: This test bot is currently in development and requires further adaptation
// to work with the specific versions of serenity, songbird, and poise used in
// the main CrackTunes codebase. The structure and design are complete, but
// compatibility with the current Discord library versions needs to be resolved.
use std::env;

use serenity::all::{ChannelId, GuildId, Http, Token, UserId};
use std::sync::Arc;

use cracktunes::test::{run_test_bot, run_test_scenario, TestHandler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();
    
    // Environment variables needed:
    // TEST_BOT_TOKEN - Discord token for the test bot
    // TARGET_BOT_ID - User ID of the CrackTunes bot to test
    
    // Optional command line arguments for specific test scenarios:
    // --guild ID - Guild ID to run tests in
    // --channel ID - Text channel ID to send commands to
    // --voice ID - Voice channel ID to join for voice tests
    
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--help" {
        println!("CrackTunes Test Bot");
        println!("Usage: {} [OPTIONS]", args[0]);
        println!("Environment variables:");
        println!("  TEST_BOT_TOKEN - Discord token for the test bot");
        println!("  TARGET_BOT_ID - User ID of the CrackTunes bot to test");
        println!("Options:");
        println!("  --help            - Show this help message");
        println!("  --run-tests       - Run specific test scenarios");
        println!("  --guild ID        - Guild ID to run tests in");
        println!("  --channel ID      - Text channel ID to send commands to");
        println!("  --voice ID        - Voice channel ID to join for voice tests");
        return Ok(());
    }
    
    // Check if we're running specific test scenarios
    if args.len() > 1 && args[1] == "--run-tests" {
        // Extract channel IDs and guild ID from command line arguments
        let mut guild_id = None;
        let mut channel_id = None;
        let mut voice_id = None;
        
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--guild" => {
                    if i + 1 < args.len() {
                        guild_id = Some(GuildId::new(args[i + 1].parse()?));
                        i += 2;
                    } else {
                        eprintln!("Missing guild ID");
                        return Ok(());
                    }
                },
                "--channel" => {
                    if i + 1 < args.len() {
                        channel_id = Some(ChannelId::new(args[i + 1].parse()?));
                        i += 2;
                    } else {
                        eprintln!("Missing channel ID");
                        return Ok(());
                    }
                },
                "--voice" => {
                    if i + 1 < args.len() {
                        voice_id = Some(ChannelId::new(args[i + 1].parse()?));
                        i += 2;
                    } else {
                        eprintln!("Missing voice channel ID");
                        return Ok(());
                    }
                },
                _ => {
                    eprintln!("Unknown option: {}", args[i]);
                    i += 1;
                }
            }
        }
        
        if let (Some(guild), Some(channel), Some(voice)) = (guild_id, channel_id, voice_id) {
            // Set up the test bot and run the specified test scenario
            let token = Token::from_env("TEST_BOT_TOKEN")?;
            let target_bot_id = env::var("TARGET_BOT_ID")?.parse::<u64>()?;
            
            // Create the test handler
            let http = Http::new(token);
            let songbird = songbird::Songbird::serenity();
            let test_handler = TestHandler::new(Arc::new(http), songbird.clone());
            test_handler.set_target_bot_id(UserId::new(target_bot_id)).await;
            
            // Run the test scenario
            match run_test_scenario(Arc::new(test_handler), channel, voice, guild).await {
                Ok(_) => println!("Test scenario completed successfully!"),
                Err(e) => eprintln!("Test scenario failed: {}", e),
            }
        } else {
            eprintln!("Missing required parameters for test scenario");
            eprintln!("Need --guild, --channel, and --voice parameters");
        }
    } else {
        // Run the test bot in interactive mode
        run_test_bot().await?;
    }
    
    Ok(())
}