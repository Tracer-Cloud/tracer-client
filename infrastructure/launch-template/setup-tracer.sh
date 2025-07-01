#!/bin/bash
set -e

echo "Starting Tracer setup at $(date)"

# Accept role ARN and API key from Terraform environment
echo "Setting up Tracer with role ARN: ${role_arn}"

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
curl -sSL https://install.tracer.cloud | bash

if ! grep -q "/root/.tracerbio/bin" /root/.bashrc; then
    echo 'export PATH="/root/.tracerbio/bin:$PATH"' >> /root/.bashrc
fi

export PATH="/root/.tracerbio/bin:$PATH"

chmod +x /root/.tracerbio/bin/tracer

echo "Tracer binary updated successfully"

if [ -d "/root/nextflow-test-pipelines" ]; then
    cd /root/nextflow-test-pipelines && git pull origin main && cd
else
    cd /root && git clone https://github.com/Tracer-Cloud/nextflow-test-pipelines.git --recurse-submodules
fi

if [ -d "/root/tracer-cleint"]; then
    cd /root/tracer-client && git pull origin main && cd
else
    cd /root && git clone https://github.com/Tracer-Cloud/tracer-client.git --recurse-submodules
fi

if [ ! -d "/root/bashrc_scripts/shell-tracer-autoinstrumentation" ]; then
    echo "Setting up workflow templates in root directory..."
    mkdir -p /root/{bashrc_scripts,nextflow_scripts,data}
        
    cp -R /tmp/temp-scripts/shell-tracer-autoinstrumentation/ /root/bashrc_scripts/
    cp -R /tmp/temp-scripts/nextflow-tracer-autoinstrumentation/ /root/nextflow_scripts/
    cp -R /tmp/temp-scripts/data/ /root/data/
    
    rm -rf /tmp/temp-scripts
    chmod -R +x /root/bashrc_scripts
    chmod -R +x /root/nextflow_scripts
    echo "Workflow templates setup completed in root directory"
fi

# Source bashrc
source ~/.bashrc

# Run tracer as root
tracer info

echo "Tracer setup completed successfully at $(date)"
echo "Bioinformatics environment is ready for use"