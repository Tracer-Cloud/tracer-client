#!/bin/bash
# installer for the tracer installer using s3
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