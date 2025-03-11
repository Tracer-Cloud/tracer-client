#!/bin/bash

# Accept role ARN and API key from terraform
ROLE_ARN="${role_arn}"
API_KEY="${api_key}"


echo "Using ROLE_ARN: $ROLE_ARN"
echo "Using API_KEY: $API_KEY"

LOG_FILE="/home/ubuntu/install_log.txt"
exec > >(tee -a "$LOG_FILE") 2>&1  # Log both stdout & stderr

echo "Starting installation at $(date)"

# Fix any broken dpkg processes
sudo dpkg --configure -a || true  # Continue if no broken packages
sudo apt clean
sudo apt autoclean

# Update package lists
sudo apt update -y

# Install all required dependencies
sudo apt install -y \
    curl \
    git \
    unzip \
    build-essential \
    pkg-config \
    libssl-dev \
    clang \
    cmake \
    gcc \
    g++ \
    zlib1g-dev \
    libclang-dev \
    openssl 

# Add Docker's official GPG key:
sudo apt-get update
sudo apt-get install ca-certificates curl
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
sudo chmod a+r /etc/apt/keyrings/docker.asc

# Add the repository to Apt sources:
echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
  $(. /etc/os-release && echo "$${UBUNTU_CODENAME:-$VERSION_CODENAME}") stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update


# Install Docker and Docker Compose
sudo apt-get install docker.io docker-compose-plugin

# Enable Docker and add `ubuntu` user to Docker group
echo "Setting up Docker..."
sudo systemctl enable docker
sudo systemctl start docker
sudo usermod -aG docker ubuntu  # Allow `ubuntu` user to run Docker without sudo

# Verify installed dependencies
pkg-config --version || echo "Error: pkg-config not installed" >> "$LOG_FILE"
dpkg -L libssl-dev | grep openssl || echo "Error: OpenSSL headers not found" >> "$LOG_FILE"


ARCH=$(uname -m)
if [ "$ARCH" = "aarch64" ]; then
    # Set environment variables for OpenSSL
    echo "Setting OpenSSL environment variables for ARM (aarch64)..."
    echo 'export OPENSSL_DIR=/usr/lib/aarch64-linux-gnu' | sudo tee -a /etc/profile
    echo 'export OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu' | sudo tee -a /etc/profile
    echo 'export OPENSSL_INCLUDE_DIR=/usr/include' | sudo tee -a /etc/profile
    echo 'export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig' | sudo tee -a /etc/profile
    source /etc/profile
else
    echo "Skipping OpenSSL config for non-aarch64 architecture: $ARCH"
fi

# Install Rust for ubuntu user
echo "Installing Rust..."
su - ubuntu -c '
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'

# Ensure Rust is installed correctly
su - ubuntu -c "source $$HOME/.cargo/env && rustc --version"

# Install GitHub CLI
echo "Installing GitHub CLI..."
type -p curl >/dev/null || sudo apt install curl -y
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
sudo chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
sudo apt update -y
sudo apt install -y gh

# Verify GitHub CLI installation
gh --version || echo "Error: GitHub CLI not installed correctly" >> "$LOG_FILE"

# Add Rust to system-wide path for immediate use
echo "export PATH=/home/ubuntu/.cargo/bin:\$${PATH}" | sudo tee /etc/profile.d/rust.sh
sudo chmod +x /etc/profile.d/rust.sh

# Clone the Tracer repository
echo "Cloning Tracer repository..."
if [ ! -d "/home/ubuntu/tracer-client" ]; then
    su - ubuntu -c "git clone https://github.com/Tracer-Cloud/tracer-client.git /home/ubuntu/tracer-client"
else
    echo "Tracer repo already exists, pulling latest changes..."
    su - ubuntu -c "cd /home/ubuntu/tracer-client && git pull"
fi

cd /home/ubuntu/tracer-client

# Install cargo-nextest
echo "Installing cargo-nextest..."
su - ubuntu -c "source $HOME/.cargo/env && cargo install --locked cargo-nextest"

# Run a nextest test to verify the installation
# echo "Running nextest..."
# su - ubuntu -c "source /home/ubuntu/.cargo/env && cd /home/ubuntu/tracer-client && cargo nextest run" || echo "Nextest failed" >> "$LOG_FILE"

# Build the Tracer binary
echo "Building Tracer..."
su - ubuntu -c "source /home/ubuntu/.cargo/env && cd /home/ubuntu/tracer-client && cargo build --release"

# Install the binary
echo "Installing Tracer binary..."
sudo cp /home/ubuntu/tracer-client/target/release/tracer /usr/local/bin/

echo "Setting Up test Environment $(date)"
su - ubuntu -c "cd /home/ubuntu/tracer-client"

echo "Running Env Setup Script"

# # NOTE: adding this line because some r dependencies aren't found at times on aws archives especially in arm
# echo "Updating sources list to use the main Ubuntu archive..."
# sudo sed -i 's|http://.*.ec2.archive.ubuntu.com/ubuntu|http://archive.ubuntu.com/ubuntu|g' /etc/apt/sources.list
# sudo apt-get update

su - ubuntu -c "cd /home/ubuntu/tracer-client && ./deployments/scripts/setup_nextflow_test_env.sh"

echo "Installation completed successfully"


echo "Setting up Tracer"
# Create the directory for the config file
mkdir -p /home/ubuntu/.config/tracer/

# Write the configuration to tracer.toml
cat <<EOL > /home/ubuntu/.config/tracer/tracer.toml
polling_interval_ms = 1500
service_url = "https://app.tracer.bio/api"
api_key = "$API_KEY"
aws_role_arn = "$ROLE_ARN"
process_polling_interval_ms = 5
batch_submission_interval_ms = 10000
new_run_pause_ms = 600000
file_size_not_changing_period_ms = 60000
process_metrics_send_interval_ms = 10000
aws_region = "us-east-2"
database_secrets_arn = "arn:aws:secretsmanager:us-east-1:395261708130:secret:rds!cluster-cd690a09-953c-42e9-9d9f-1ed0b434d226-M0wZYA"
database_host = "tracer-cluster-v2-instance-1.cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432"
database_name = "tracer_db"
EOL

echo "Configuration file created at /home/ubuntu/.config/tracer/tracer.toml"

source ~/.bashrc

echo "Tracer setup successfully $(date)"