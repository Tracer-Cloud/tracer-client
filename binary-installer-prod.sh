#!/bin/bash
# installer for the tracer installer github need to update based on dev installer

curl -sSL https://install.tracer.cloud/binary-installer-common.sh -o /tmp/binary-installer-common.sh
. /tmp/binary-installer-common.sh

USER_ID="$1"

# Get the latest release version from GitHub API
echo "$(colorize "[SEARCHING]" "blue") Fetching latest release version..."
VERSION=$(curl -s https://api.github.com/repos/Tracer-Cloud/tracer-client/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [[ -z "$VERSION" ]]; then
    echo "$(error) Failed to fetch latest version from GitHub API"
    echo "$(colorize "[INFO]" "cyan") Falling back to hardcoded version..."
    VERSION="v2025.6.18+1"
fi

DOWNLOAD_URL="https://github.com/Tracer-Cloud/tracer-client/releases/download/$VERSION"

fetch_execute_installer "$DOWNLOAD_URL" "$USER_ID" || exit 1
