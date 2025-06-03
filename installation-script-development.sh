#!/bin/bash

# Development installation script
# This script downloads and executes the main installation script with production parameter

# URL to the main installation script
INSTALL_SCRIPT_URL="https://raw.githubusercontent.com/Tracer-Cloud/tracer-client/refs/heads/dev/installation.sh"

# Download and execute the installation script with production parameter
curl -sSL "$INSTALL_SCRIPT_URL" | bash -s development

