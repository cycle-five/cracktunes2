#!/bin/bash
# Script to set up the development environment

set -e

# Check if Rust is installed
if ! command -v rustc &> /dev/null; then
    echo "Rust is not installed. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
else
    echo "Rust is already installed."
fi

# Check Rust version
RUST_VERSION=$(rustc --version | cut -d ' ' -f 2)
REQUIRED_VERSION="1.85.0"

if [ "$(printf '%s\n' "$REQUIRED_VERSION" "$RUST_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]; then
    echo "Updating Rust to required version..."
    rustup update stable
    rustup default stable
fi

# Install system dependencies
if command -v apt-get &> /dev/null; then
    echo "Installing system dependencies with apt..."
    sudo apt-get update
    sudo apt-get install -y pkg-config libopus-dev ffmpeg python3
elif command -v brew &> /dev/null; then
    echo "Installing system dependencies with Homebrew..."
    brew install opus ffmpeg python3
elif command -v pacman &> /dev/null; then
    echo "Installing system dependencies with pacman..."
    sudo pacman -Sy --noconfirm opus ffmpeg python
else
    echo "Could not determine package manager. Please install the following dependencies manually:"
    echo "- Opus development libraries"
    echo "- FFmpeg"
    echo "- Python 3"
fi

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "Creating .env file from example..."
    cp .env.example .env
    echo "Please edit .env and add your Discord token."
fi

# Build the project
echo "Building the project..."
cargo build

echo "Development environment setup complete!"
echo "Run 'cargo test' to run tests."
echo "Run 'cargo run' to start the bot (after setting DISCORD_TOKEN in .env)."
