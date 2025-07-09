#!/bin/bash
# installer for the tracer installer using s3

# Define emoji fallbacks
EMOJI_SEARCH="🔍 "
EMOJI_CANCEL="❌ "
EMOJI_CLIPBOARD="📋 "
EMOJI_PACKAGE="📦 "

# Use fallback for terminals that don't support emojis
if ! [[ "$TERM" =~ ^xterm.* || "$TERM" == "screen" ]]; then
  EMOJI_SEARCH="[SEARCH] "
  EMOJI_CANCEL="[ERROR] "
  EMOJI_CLIPBOARD="[INFO] "
  EMOJI_PACKAGE="[DOWNLOAD] "
fi

# Function to send Sentry alert
send_sentry_alert() {
  local message="$1"
  local level="${2:-info}"

  local DSN="https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"

  # Parse DSN components
  local proto="${DSN%%:*}"
  local tmp="${DSN#*://}"
  local public_key="${tmp%%@*}"
  tmp="${tmp#*@}"
  local host="${tmp%%/*}"
  local project_id="${tmp##*/}"

  # Compose the API URL for sending events
  local url="${proto}://${host}/api/${project_id}/store/?sentry_version=7&sentry_key=${public_key}"

  # Detect OS and version
  local os=""
  local arch
  arch="$(uname -m)"

  if [[ "$(uname)" == "Darwin" ]]; then
    # macOS
    local product_name product_version
    product_name=$(sw_vers -productName)
    product_version=$(sw_vers -productVersion)
    os="${product_name} ${product_version}"
  elif [[ -f /etc/os-release ]]; then
    # Linux
    # shellcheck disable=SC1091
    source /etc/os-release
    os="${NAME} ${VERSION_ID:-$VERSION}"
  else
    # Fallback generic
    os="$(uname -s) $(uname -r)"
  fi

  # Compose JSON payload with tags
  local payload
  payload=$(printf '{"message":"%s","level":"%s","platform":"bash","tags":{"os":"%s","arch":"%s"}}' \
    "$message" "$level" "$os" "$arch")

  # Send the event
  curl -sS -f -o /dev/null \
       -H "Content-Type: application/json" \
       -d "$payload" \
       -X POST "$url"
}

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

REPO_URL="https://github.com/Tracer-Cloud/tracer-client/releases/download/${VERSION}/{$BINARY_NAME}"

# Map to download URL based on platform
case "$OS" in
  Linux*)

  # Check glibc version requirement (minimum 2.34)
    GLIBC_VERSION=$(ldd --version 2>&1 | head -n1 | grep -oE '[0-9]+\.[0-9]+' | head -n1)

    if [[ -z "$GLIBC_VERSION" ]]; then
      echo "${EMOJI_CANCEL}Could not determine glibc version"
      exit 1
    fi

    GLIBC_MAJOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f1)
    GLIBC_MINOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f2)

    echo "${EMOJI_CLIPBOARD}Detected glibc version: $GLIBC_VERSION"

    # Check if glibc is at least 2.34
    if [ "$GLIBC_MAJOR" -lt 2 ] || ([ "$GLIBC_MAJOR" -eq 2 ] && [ "$GLIBC_MINOR" -lt 34 ]); then
      send_sentry_alert "Unsupported glibc version: $GLIBC_VERSION on $(uname -a)." "info"

      echo "${EMOJI_CANCEL}Linux support requires GLIBC version >= 2.36. Detected GLIBC version: $GLIBC_VERSION.
        Tested on Ubuntu 22.04 and Amazon Linux 2023.
        Please update your Linux distribution, or contact support@tracer.cloud if Tracer is not working with your preferred distribution."
      exit 1
    fi

    case "$ARCH" in
      x86_64)
        DOWNLOAD_URL="$REPO_URL-x86_64-unknown-linux-gnu.tar.gz"
        ;;
      aarch64)
        DOWNLOAD_URL="$REPO_URL-aarch64-unknown-linux-gnu.tar.gz"
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
        DOWNLOAD_URL="$REPO_URL-x86_64-apple-darwin.tar.gz"
        ;;
      arm64)
        DOWNLOAD_URL="$REPO_URL-aarch64-apple-darwin.tar.gz"
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