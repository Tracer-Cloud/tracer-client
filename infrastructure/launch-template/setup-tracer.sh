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
rm -f /usr/local/bin/tracer
curl -sSL https://install.tracer.cloud | bash

cp /root/.tracerbio/bin/tracer /usr/local/bin/
chmod +x /usr/local/bin/tracer

echo "Tracer binary updated successfully"

# Check if bioinformatics pipeline repository exists
if [ -d "/root/tracer-test-pipelines-bioinformatics" ]; then
    echo "Bioinformatics pipeline repository found, updating..."
    cd /root/tracer-test-pipelines-bioinformatics && git pull origin main && cd
    echo "Bioinformatics pipeline updated successfully"
else
    echo "Bioinformatics pipeline repository not found, cloning..."
    cd /root && git clone https://github.com/Tracer-Cloud/tracer-test-pipelines-bioinformatics.git --recurse-submodules
    echo "Bioinformatics pipeline cloned successfully"
fi

# Check if workflow templates exist in root and create if needed
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