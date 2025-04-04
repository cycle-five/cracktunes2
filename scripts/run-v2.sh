#!/bin/bash

# Run the bot with the new hexagonal architecture
# Make sure to set DISCORD_TOKEN before running or pass as an environment variable

# Get token from environment or prompt for it
if [ -z "$DISCORD_TOKEN" ]; then
    echo "Please provide your Discord bot token:"
    read -rs DISCORD_TOKEN
    export DISCORD_TOKEN
fi

# Run the bot with the new architecture
cargo run --bin cracktunes-v2