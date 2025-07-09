#!/bin/bash installer for the tracer installer using s3

# Get optional user_id from the first positional argument
USER_ID="$1"

# Determine OS and ARCH
OS=$(uname -s)
ARCH=$(uname -m)

# Define binary name

# Get the latest release version from GitHub API
REPO_URL="https://tracer-installer-releases.s3.us-east-1.amazonaws.com"

# Map to download URL based on platform
case "$OS" in
  Linux*)
    case "$ARCH" in
      x86_64)
        DOWNLOAD_URL="$REPO_URL/x86_64-unknown-linux-gnu.tar.gz"
        ;;
      aarch64)
        DOWNLOAD_URL="$REPO_URL/aarch64-unknown-linux-gnu.tar.gz"
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
        DOWNLOAD_URL="$REPO_URL/x86_64-apple-darwin.tar.gz"
        ;;
      arm64)
        DOWNLOAD_URL="$REPO_URL/aarch64-apple-darwin.tar.gz"
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
echo "\n"
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

# Run the binary with or without user ID
if [[ -n "$USER_ID" ]]; then
  sudo "$EXTRACT_DIR/$BINARY_NAME" run --user-id="$USER_ID"
else
  sudo "$EXTRACT_DIR/$BINARY_NAME" run
fi