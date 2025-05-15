#!/bin/bash
set -e

echo "Updating Repositories $(date)"
cd /root/tracer-test-pipelines-bioinformatics && git pull origin main && cd
echo "Repositories Updated Successfully $(date)"

# Accept role ARN and API key from Terraform environment
echo "Setting up Tracer"

# Create the config directory
mkdir -p /root/.config/tracer/

# Setup /tmp/tracer with correct permissions
echo "Setting up /tmp/tracer directory and permissions..."
groupadd -f tracer
usermod -aG tracer root

mkdir -p /tmp/tracer
chown root:tracer /tmp/tracer
chmod 2775 /tmp/tracer

# Install Tracer binary as root
echo "Updating Tracer binary..."
rm -f /usr/local/bin/tracer
curl -sSL https://install.tracer.cloud | bash

cp /root/.tracerbio/bin/tracer /usr/local/bin/
chmod +x /usr/local/bin/tracer

echo "Tracer binary updated successfully"

# Source bashrc
source ~/.bashrc

# Run tracer as root
tracer info

echo "Tracer setup successfully at $(date)"