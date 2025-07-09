#!/bin/bash
# installer for the tracer installer using s3

# Function to send Sentry alert
# send_sentry_alert() {
#     local message="$1"
#     local level="${2:-info}"

#     SENTRY_DSN="https://add417a1c944b1b2110b4f3ea8d7fbea@o4509525906948096.ingest.de.sentry.io/4509530452328528"

#     if [[ -n "$SENTRY_DSN" ]]; then
#         curl -X POST "$SENTRY_DSN" \
#             -H "Content-Type: application/json" \
#             -d "{\"message\": \"$message\", \"level\": \"$level\", \"platform\": \"bash\", \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)\"}" \
#             --silent --show-error || true
#     else
#         echo "🔔 Sentry Alert: [$level] $message"
#     fi
# }

USER_ID="$1"
CLIENT_BRANCH="${CLI_BRANCH:-}"
INSTALLER_BRANCH="${INS_BRANCH:-}"
# Determine OS and ARCH
OS=$(uname -s)
ARCH=$(uname -m)

# Define binary name
BINARY_NAME="tracer-installer"

# S3 repository URL for dev releases
if [[ -n "$INSTALLER_BRANCH" ]]; then
  echo "Using installer branch: $INSTALLER_BRANCH"
else
  INSTALLER_BRANCH="main"
fi


REPO_URL="https://tracer-installer-releases.s3.us-east-1.amazonaws.com/${INSTALLER_BRANCH}"




# Map to download URL based on platform
case "$OS" in
  Linux*)
    # Check glibc version requirement (minimum 2.34)
    echo "🔍 Checking glibc version..."
    GLIBC_VERSION=$(ldd --version 2>&1 | head -n1 | grep -oE '[0-9]+\.[0-9]+' | head -n1)

    if [[ -z "$GLIBC_VERSION" ]]; then
      echo "❌ Could not determine glibc version"
      exit 1
    fi

    GLIBC_MAJOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f1)
    GLIBC_MINOR=$(echo "$GLIBC_VERSION" | cut -d'.' -f2)

    echo "📋 Detected glibc version: $GLIBC_VERSION"

    # Check if glibc is at least 2.34
    if [ "$GLIBC_MAJOR" -lt 2 ] || ([ "$GLIBC_MAJOR" -eq 2 ] && [ "$GLIBC_MINOR" -lt 34 ]); then
      # send_sentry_alert "Unsupported glibc version: $GLIBC_VERSION on $(uname -a). Linux support requires GLIBC version >= 2.34; detected GLIBC version: $GLIBC_VERSION. Tested on Ubuntu 22.04 and Amazon Linux 2023. Please report if Tracer does not work with your preferred Linux distribution." "error"

      echo "❌ glibc version $GLIBC_VERSION is not supported. Minimum required: 2.34"
      echo "🔄 Please upgrade your system to a newer version with glibc 2.34 or later"
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

if command -v sudo >/dev/null 2>&1; then
  INVOKER=(sudo)
elif [[ $(id -u) -eq 0 ]]; then
  INVOKER=()         # already root, no sudo needed
else
  echo "Rerun this script with root privileges or use sudo." >&2
  exit 1
fi

cmd=("${INVOKER[@]}" "$EXTRACT_DIR/$BINARY_NAME" run)

[[ -n "$CLIENT_BRANCH" ]] && cmd+=(--channel="$CLIENT_BRANCH")
[[ -n "$USER_ID"      ]] && cmd+=(--user-id="$USER_ID")

echo "${cmd[@]}"
"${cmd[@]}"