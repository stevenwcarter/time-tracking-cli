#!/bin/bash

# Install script for time-tracking-cli
# This script creates a symlink from 'tt' to 'ttcli' in the cargo bin directory

set -e

cd site

yarn

yarn build

cd ..

cargo install --path cli


# Get the cargo bin directory
CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

# Check if ttcli exists
if [ ! -f "$CARGO_BIN_DIR/ttcli" ]; then
    echo "Error: ttcli not found in $CARGO_BIN_DIR"
    echo "Please install the crate first with: cargo install --path cli"
    exit 1
fi

# Create symlink
if [ -L "$CARGO_BIN_DIR/tt" ]; then
    echo "Removing existing tt symlink..."
    rm "$CARGO_BIN_DIR/tt"
elif [ -f "$CARGO_BIN_DIR/tt" ]; then
    echo "Warning: $CARGO_BIN_DIR/tt exists and is not a symlink"
    echo "Please remove it manually if you want to replace it"
    exit 1
fi

echo "Creating symlink: tt -> ttcli"
ln -s "$CARGO_BIN_DIR/ttcli" "$CARGO_BIN_DIR/tt"

echo "✅ Installation complete!"
echo "You can now use both 'tt' and 'ttcli' commands"
