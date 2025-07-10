#!/bin/sh
# installer for the tracer installer using s3

# Define emoji fallbacks
EMOJI_SEARCH="🔍 "
EMOJI_CANCEL="❌ "
EMOJI_CLIPBOARD="📋 "
EMOJI_PACKAGE="📦 "

# Use fallback for terminals that don't support emojis
case "$TERM" in
  xterm*|screen)
    # Keep emoji defaults
    ;;
  *)
    EMOJI_SEARCH="[SEARCH] "
    EMOJI_CANCEL="[ERROR] "
    EMOJI_CLIPBOARD="[INFO] "
    EMOJI_PACKAGE="[DOWNLOAD] "
    ;;
esac

# Determine OS and ARCH
OS=$(uname -s)
ARCH=$(uname -m)
OS_FULL=""
# Detect OS and version
if [ "$(uname)" = "Darwin" ]; then
  # macOS
  os_version=$(sw_vers -productVersion)
  OS_FULL="macOS (Darwin) ${os_version}"
elif [ -f /etc/os-release ]; then
  # Linux
  # shellcheck disable=SC1091
  . /etc/os-release
  OS_FULL="${NAME} ${VERSION_ID:-${VERSION}}"
else
  # Fallback generic
  OS_FULL="$(uname -s) $(uname -r)"
fi

# Function to send Sentry alert
send_sentry_alert() {
  message="$1"
  level="${2:-info}"

  DSN="https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"

  # Parse DSN components
  proto="${DSN%%:*}"
  tmp="${DSN#*://}"
  public_key="${tmp%%@*}"
  tmp="${tmp#*@}"
  host="${tmp%%/*}"
  project_id="${tmp##*/}"

  # Compose the API URL for sending events
  url="${proto}://${host}/api/${project_id}/store/?sentry_version=7&sentry_key=${public_key}"

  # Compose JSON payload with tags
  payload=$(printf '{"message":"%s","level":"%s","platform":"sh","tags":{"os":"%s","arch":"%s"}}' \
    "$message" "$level" "$OS_FULL" "$ARCH")

  # Send the event
  curl -sS -f -o /dev/null \
       -H "Content-Type: application/json" \
       -d "$payload" \
       -X POST "$url"
}

USER_ID="$1"
CLIENT_BRANCH="${CLI_BRANCH:-}"
INSTALLER_BRANCH="${INS_BRANCH:-}"


# Define binary name
BINARY_NAME="tracer-installer"

# S3 repository URL for dev releases
if [ -n "$INSTALLER_BRANCH" ]; then
  echo "Using installer branch: $INSTALLER_BRANCH"
else
  INSTALLER_BRANCH="main"
fi


REPO_URL="https://tracer-installer-releases.s3.us-east-1.amazonaws.com/${INSTALLER_BRANCH}"




# Map to download URL based on platform
case "$OS" in
  Linux*)
    # Check glibc version requirement (minimum 2.34)
    GLIBC_VERSION=$(ldd --version 2>&1 | head -n1 | sed -n 's/.*\([0-9][0-9]*\.[0-9][0-9]*\).*/\1/p')

    if [ -z "$GLIBC_VERSION" ]; then
      echo "${EMOJI_CANCEL}Could not determine glibc version"
      exit 1
    fi

    GLIBC_MAJOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f1)
    GLIBC_MINOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f2)

    if [ "$GLIBC_MAJOR" -lt 2 ] || ([ "$GLIBC_MAJOR" -eq 2 ] && [ "$GLIBC_MINOR" -lt 34 ]); then
      send_sentry_alert "Unsupported glibc version: $GLIBC_VERSION on $OS_FULL." "info"

      echo "${EMOJI_CANCEL} Linux support requires GLIBC version >= 2.36. Detected GLIBC version: $GLIBC_VERSION."
      echo "Tested on Ubuntu 22.04 and Amazon Linux 2023."
      echo "Please update your Linux distribution, or contact support@tracer.cloud if Tracer is not working with your preferred distribution."
      exit 1
    fi

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
    send_sentry_alert "Unsupported operating system: $OS_FULL." "info"
    exit 1
    ;;
esac

# Download, extract, and run
TEMP_DIR=$(mktemp -d)
ARCHIVE_PATH="$TEMP_DIR/${BINARY_NAME}.tar.gz"
EXTRACT_DIR="$TEMP_DIR/extracted"

mkdir -p "$EXTRACT_DIR"
echo "\n"
echo "${EMOJI_PACKAGE}Downloading Tracer Installer from: $DOWNLOAD_URL"
curl -L "$DOWNLOAD_URL" -o "$ARCHIVE_PATH" || {
  echo "${EMOJI_CANCEL}Failed to download binary"
  send_sentry_alert "Failed to download binary from $DOWNLOAD_URL." "info"
  exit 1
}


tar -xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIR" || {
  echo "${EMOJI_CANCEL}Failed to extract archive"
  exit 1
}

chmod +x "$EXTRACT_DIR/$BINARY_NAME"

# Run the binary with or without user ID

if command -v sudo >/dev/null 2>&1; then
  INVOKER="sudo"
elif [ "$(id -u)" -eq 0 ]; then
  INVOKER=""         # already root, no sudo needed
else
  echo "Rerun this script with root privileges or use sudo." >&2
  exit 1
fi

# Build command
if [ -n "$INVOKER" ]; then
  cmd="$INVOKER $EXTRACT_DIR/$BINARY_NAME run"
else
  cmd="$EXTRACT_DIR/$BINARY_NAME run"
fi

if [ -n "$CLIENT_BRANCH" ]; then
  cmd="$cmd --channel=$CLIENT_BRANCH"
fi

if [ -n "$USER_ID" ]; then
  cmd="$cmd --user-id=$USER_ID"
fi

echo "$cmd"
eval "$cmd"