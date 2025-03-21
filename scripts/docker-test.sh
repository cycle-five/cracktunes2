#!/bin/bash
# Script to test the Docker setup locally

set -e

echo "Building Docker image..."
docker build -t cracktunes:test .

echo "Running tests inside Docker container..."
docker run --rm cracktunes:test cargo test --all

echo "Docker tests completed successfully!"
