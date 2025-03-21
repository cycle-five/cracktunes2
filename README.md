# CrackTunes

A Discord music bot written in Rust that allows users to play music from YouTube and Spotify in Discord voice channels.

## Features

- Play music from YouTube and Spotify
- Queue management
- Playlist support
- Skip, stop, and shuffle commands
- Docker support
- CI/CD pipeline with GitHub Actions

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

### Using Docker Compose

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

This script builds a Docker image and runs the tests inside the container.

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

- `/ping`: Pings the bot
- `/join`: Joins the voice channel
- `/leave`: Leaves the voice channel
- `/play_fade`: Plays a song with a fade effect
- `/queue`: Adds a song to the queue
- `/skip`: Skips the current song
- `/stop`: Stops playback and clears the queue
- `/show_queue`: Displays the current queue
- `/shuffle`: Shuffles the queue
- `/mute`: Mutes the bot
- `/unmute`: Unmutes the bot
- `/deafen`: Deafens the bot
- `/undeafen`: Undeafens the bot

### Slash Commands vs Prefix Commands

With the direction that Discord is pushing with regards to applications like cracktunes on their
platform, prefix commands are no longer supported, nor likely will be supported again.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
