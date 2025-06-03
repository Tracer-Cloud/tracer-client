#!/bin/bash

# Development installation script
# This script downloads and executes the main installation script with development parameter

# URL to the main installation script
INSTALL_SCRIPT_URL="https://raw.githubusercontent.com/Tracer-Cloud/tracer-client/refs/heads/dev/installation.sh"

# Download and execute the installation script with development parameter
curl -sSL "$INSTALL_SCRIPT_URL" | bash -s development

