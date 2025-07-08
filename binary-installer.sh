#!/bin/bash

# Get optional user_id from the first positional argument
USER_ID="$1"

# Determine OS and ARCH
OS=$(uname -s)
ARCH=$(uname -m)

# Define binary name
BINARY_NAME="tracer-installer"

# Get the latest release version from GitHub API
echo "🔍 Fetching latest release version..."
VERSION=$(curl -s https://api.github.com/repos/Tracer-Cloud/tracer-client/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [[ -z "$VERSION" ]]; then
    echo "❌ Failed to fetch latest version from GitHub API"
    echo "🔄 Falling back to hardcoded version..."
    VERSION="v2025.6.18+1"
fi

REPO_URL="https://github.com/Tracer-Cloud/tracer-client/releases/download/${VERSION}"

# Map to download URL based on platform
case "$OS" in
  Linux*)
    case "$ARCH" in
      x86_64)
        DOWNLOAD_URL="$REPO_URL/${BINARY_NAME}-x86_64-unknown-linux-gnu.tar.gz"
        ;;
      aarch64)
        DOWNLOAD_URL="$REPO_URL/${BINARY_NAME}-aarch64-unknown-linux-gnu.tar.gz"
        ;;
      *)
        echo "Unsupported Linux architecture: $ARCH"
        exit 1
        ;;
    esac
    ;;
  Darwin*)
    case "$ARCH" in
      x86_64)
        DOWNLOAD_URL="$REPO_URL/${BINARY_NAME}-x86_64-apple-darwin.tar.gz"
        ;;
      arm64)
        DOWNLOAD_URL="$REPO_URL/${BINARY_NAME}-aarch64-apple-darwin.tar.gz"
        ;;
      *)
        echo "Unsupported macOS architecture: $ARCH"
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported operating system: $OS"
    exit 1
    ;;
esac

# Download, extract, and run
TEMP_DIR=$(mktemp -d)
ARCHIVE_PATH="$TEMP_DIR/${BINARY_NAME}.tar.gz"
EXTRACT_DIR="$TEMP_DIR/extracted"

mkdir -p "$EXTRACT_DIR"
echo "📦 Downloading Tracer Installer from: $DOWNLOAD_URL"
curl -L "$DOWNLOAD_URL" -o "$ARCHIVE_PATH" || {
  echo "❌ Failed to download binary"
  exit 1
}

tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR" || {
  echo "❌ Failed to extract archive"
  exit 1
}

chmod +x "$EXTRACT_DIR/$BINARY_NAME"
echo "🚀 Executing Tracer Installer..."

# Run the binary with or without user ID
if [[ -n "$USER_ID" ]]; then
  sudo "$EXTRACT_DIR/$BINARY_NAME" run --user-id="$USER_ID"
else
  sudo "$EXTRACT_DIR/$BINARY_NAME" run
fi