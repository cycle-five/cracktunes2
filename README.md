# CrackTunes 🎵

[![Version](https://img.shields.io/badge/version-0.4.0-blue.svg)](https://github.com/cycle-five/cracktunes/releases)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85.0+-orange.svg)](https://www.rust-lang.org/)
[![Discord](https://img.shields.io/badge/discord-bot-7289DA.svg)](https://cracktun.es/)

A high-performance Discord music bot written in Rust that brings YouTube and Spotify music to your Discord voice channels. CrackTunes offers a robust, thread-safe implementation with comprehensive queue management capabilities and containerized deployment options.

## ✨ Features

### Music Sources
- **YouTube** - Play songs, playlists, and perform searches
- **Spotify** - Stream tracks and playlists from Spotify

### Playback Control
- **Rich Queue Management** - Add, remove, clear, shuffle, and display tracks
- **Playback Controls** - Play, pause, skip, stop
- **Audio Controls** - Adjust volume, mute/unmute, deafen/undeafen

### Technical Features
- **Thread-safe Implementation** - Concurrent access to music queues
- **Docker Support** - Containerized deployment for easy hosting
- **CI/CD Pipeline** - Automated testing and deployment with GitHub Actions
- **Slash Commands** - Modern Discord command integration
- **Integration Testing** - Companion test bot for automated testing

## Prerequisites

- Rust 1.85.0 or later
- Discord Bot Token
- Docker (optional, for containerized deployment)

## Development Setup

### Automatic Setup

We provide a setup script that installs all dependencies and prepares your development environment:

```bash
./scripts/dev-setup.sh
```

This script will:
- Install or update Rust to the required version
- Install system dependencies
- Create a `.env` file from the example
- Build the project

### Manual Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/cycle-five/cracktunes.git
   cd cracktunes
   ```

2. Install dependencies:
   - Rust 1.85.0 or later
   - OpenSSL development libraries
   - Opus development libraries
   - FFmpeg
   - Python 3

3. Build the project:
   ```bash
   cargo build
   ```

4. Run the tests:
   ```bash
   cargo test
   ```

5. Run the bot (requires a Discord token):
   ```bash
   DISCORD_TOKEN=your_token_here cargo run
   ```

## Docker Deployment

### Using Docker Compose (Recommended)

1. Create a `.env` file with your Discord token:
   ```
   DISCORD_TOKEN=your_token_here
   ```

2. Build and start the container:
   ```bash
   docker-compose up -d
   ```

3. View logs:
   ```bash
   docker-compose logs -f
   ```

4. Stop the container:
   ```bash
   docker-compose down
   ```

### Using Docker Directly

1. Build the Docker image:
   ```bash
   docker build -t cracktunes .
   ```

2. Run the container:
   ```bash
   docker run -d --name cracktunes -e DISCORD_TOKEN=your_token_here cracktunes
   ```

### Testing Docker Setup

We provide a script to test the Docker setup:

```bash
./scripts/docker-test.sh
```

This script builds a Docker image and runs the tests inside the container, ensuring your environment is correctly configured.

## Integration Testing

CrackTunes includes a test bot framework that can be used to perform integration tests on the main bot. This companion bot can:

- Send commands and verify responses
- Join voice channels to listen to audio output
- Verify volume changes and detect audio patterns
- Run automated test scenarios

### Setting Up the Test Bot

1. Register a separate Discord bot to use as the test bot
2. Set up environment variables:
   ```bash
   export TEST_BOT_TOKEN=your_test_bot_token
   export TARGET_BOT_ID=your_cracktunes_bot_user_id
   ```

3. Enable the test-bot feature in your build:
   ```bash
   cargo build --features test-bot
   ```

### Test Bot Architecture

The test bot has several main components:

- **TestHandler**: Main bot handler for processing events and managing test expectations
- **AudioAnalyzer**: Processes audio data from voice channels to detect patterns and volume changes
- **TestExpectation**: Tracks the expected responses for commands

The integration test environment allows for:
- Sending commands to the music bot and verifying responses
- Monitoring voice state changes
- Analyzing audio output to detect volume changes or specific patterns
- Running automated test scenarios with predefined expectations

#### Implementing Custom Tests

To create custom test scenarios, use the `TestHandler` to:

1. Set up test expectations with `add_expectation()`
2. Send commands to the music bot with `send_command()`
3. Verify results with `get_test_results()`

For audio testing, use the `AudioAnalyzer` to:
1. Listen for audio data in voice channels
2. Detect volume changes with `get_volume_changes()`
3. Check for specific audio patterns with `get_detected_patterns()`

## CI/CD Pipeline

This project uses GitHub Actions for continuous integration and deployment:

1. **Daily Tests**: Runs all tests every day at 2:00 UTC to ensure ongoing stability.

2. **Pull Request Checks**: When a pull request is opened against the main, master, or develop branches, the workflow:
   - Checks code formatting
   - Runs clippy for linting
   - Builds the project
   - Runs all tests
   - Builds a Docker image

3. **Release Pipeline**: When code is pushed to the main or master branch, the workflow:
   - Runs all tests
   - Builds a release binary
   - Creates a GitHub release with the version from Cargo.toml
   - Builds and pushes a Docker image to GitHub Container Registry

### Discord Notifications

The CI/CD pipelines send notifications to Discord for important events:
- Test failures in daily tests
- Pull request check results (success or failure)
- New releases

To set up Discord notifications:

1. Create a webhook in your Discord server:
   - Go to Server Settings > Integrations > Webhooks
   - Click "New Webhook"
   - Name it (e.g., "GitHub CI/CD")
   - Choose the channel for notifications
   - Copy the webhook URL

2. Add the webhook URL as a secret in your GitHub repository:
   - Go to your repository on GitHub
   - Navigate to Settings > Secrets and variables > Actions
   - Click "New repository secret"
   - Name: `DISCORD_WEBHOOK`
   - Value: Your Discord webhook URL
   - Click "Add secret"

## Bot Commands

### Connection Management
- `/ping`: Check if the bot is responsive
- `/join`: Join your current voice channel
- `/leave`: Leave the voice channel

### Playback Controls
- `/play <url|search query>`: Play a song from YouTube or Spotify (URL or search)
- `/playnext <url|search query>`: Add a song to the front of the queue.
- `/skip`: Skip to the next song in the queue
- `/stop`: Stop playback and clear the queue
- `/show_queue`: Display all songs currently in the queue
- `/shuffle`: Randomize the order of songs in the queue

### Audio Settings
- `/mute`: Mute the bot's audio output
- `/unmute`: Unmute the bot's audio output
- `/deafen`: Deafen the bot (both mutes output and stops receiving audio)
- `/undeafen`: Undeafen the bot

### Playlist Management
- `/playlist <commands>`: Manage saved playlists (create, add, load, etc.)

### Slash Commands vs Prefix Commands

With the direction that Discord is pushing for applications like CrackTunes on their
platform, prefix commands are no longer supported, nor likely will be supported again.
CrackTunes exclusively uses Discord's slash command system for all interactions.

## Architecture

CrackTunes is built with a focus on performance and reliability:

- **Queue System**: Thread-safe implementation using `Arc<Mutex<>>` allows concurrent access to track queues
- **Track Resolution**: Support for multiple music sources with a unified resolution system
- **Event Handling**: Robust event system for handling Discord events and playback state changes
- **Dockerized Deployment**: Containerized for consistent deployment across environments
- **Integration Testing**: Companion test bot framework to verify functionality

## Troubleshooting

### Common Issues

1. **Bot doesn't respond to commands**
   - Ensure your bot has the correct permissions in Discord
   - Verify your Discord token is correct in the `.env` file
   - Check the logs for any error messages

2. **Audio playback issues**
   - Ensure ffmpeg is properly installed if running locally
   - Check if the bot has permission to join voice channels
   - Verify your internet connection is stable

3. **Docker deployment issues**
   - Make sure Docker and Docker Compose are up-to-date
   - Check if the `.env` file is properly configured
   - Review container logs for detailed error information

## Contributing

Contributions are welcome! Here's how you can help:

1. **Report bugs** - Create an issue describing the bug and steps to reproduce
2. **Suggest features** - Open an issue with your feature suggestion
3. **Submit pull requests** - Fork the repository, make your changes, and submit a PR

Please adhere to the existing code style and include tests for new functionality.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

Developed by [Cycle Five](https://github.com/cycle-five)