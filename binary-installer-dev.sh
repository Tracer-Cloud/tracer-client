#!/bin/sh
# installer for the tracer installer using s3

curl -sSL https://250a23db.tracer-client.pages.dev/binary-installer-common.sh -o /tmp/binary-installer-common.sh
. /tmp/binary-installer-common.sh

USER_ID="$1"
CLIENT_BRANCH="${CLI_BRANCH:-}"
INSTALLER_BRANCH="${INS_BRANCH:-}"

# S3 repository URL for dev releases
if [ -n "$INSTALLER_BRANCH" ]; then
  echo "Using installer branch: $INSTALLER_BRANCH"
else
  INSTALLER_BRANCH="main"
fi

DOWNLOAD_URL="https://tracer-installer-releases.s3.us-east-1.amazonaws.com/$INSTALLER_BRANCH"

fetch_execute_installer "$DOWNLOAD_URL" "$USER_ID" "$CLIENT_BRANCH"|| exit 1
