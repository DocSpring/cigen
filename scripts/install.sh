#!/usr/bin/env bash
set -e

echo "Installing cigen..."
cargo install --path . --force

echo ""
echo "✅ cigen installed successfully!"
echo "Location: $HOME/.cargo/bin/cigen"
echo ""
echo "Make sure $HOME/.cargo/bin is in your PATH"
