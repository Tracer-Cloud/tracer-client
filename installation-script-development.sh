#!/bin/bash

# Development installation script
# This script downloads and executes the main installation script with branch + user ID

# URL to the main installation script
INSTALL_SCRIPT_URL="https://install.tracer.cloud/installation.sh"
# Get the branch name from the first argument, default to "development"
BRANCH_NAME=${1:-"development"}

# Download and execute the installation script with the branch name parameter
curl -sSL "$INSTALL_SCRIPT_URL" | bash -s "$BRANCH_NAME"