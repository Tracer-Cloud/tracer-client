#!/bin/bash


sed -i 's/\[ -z "\$PS1" \]/[ -z "$${PS1-}" ]/' /root/.bashrc || true

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
    openssl \
    ca-certificates \
    clang \
    libelf1 \
    libelf-dev \
    zlib1g-dev

echo "Installing docker"
curl -fsSL https://get.docker.com -o get-docker.sh
# No need for newgrp, it doesn't persist in scripts

ARCH=$(uname -m)
if [ "$ARCH" = "aarch64" ]; then
    echo "Setting OpenSSL environment variables for ARM (aarch64)..."
    echo 'export OPENSSL_DIR=/usr/lib/aarch64-linux-gnu' | sudo tee -a /etc/profile
    echo 'export OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu' | sudo tee -a /etc/profile
    echo 'export OPENSSL_INCLUDE_DIR=/usr/include' | sudo tee -a /etc/profile
    echo 'export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig' | sudo tee -a /etc/profile
    source /etc/profile
else
    echo "Skipping OpenSSL config for non-aarch64 architecture: $ARCH"
fi

echo "Installing Rust..."
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Source Rust environment for current shell
source /root/.cargo/env
rustc --version

# Add Rust to system-wide PATH
echo 'export PATH=/root/.cargo/bin:$PATH' > /etc/profile.d/rust.sh
chmod +x /etc/profile.d/rust.sh

echo "Installing GitHub CLI..."
type -p curl >/dev/null || sudo apt install curl -y
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
sudo chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
sudo apt update -y
sudo apt install -y gh
gh --version || echo "Error: GitHub CLI not installed correctly" >> "$LOG_FILE"

echo "export PATH=/home/ubuntu/.cargo/bin:\$PATH" | sudo tee /etc/profile.d/rust.sh
sudo chmod +x /etc/profile.d/rust.sh

echo "Cloning Tracer repository..."
if [ ! -d "/root/tracer-client" ]; then
    git clone https://github.com/Tracer-Cloud/tracer-client.git /root/tracer-client
else
    echo "Tracer repo already exists, pulling latest changes..."
    cd /root/tracer-client && git pull
fi

echo "Setting up /tmp/tracer directory and permissions..."
groupadd -f tracer
usermod -aG tracer ubuntu
usermod -aG tracer root
mkdir -p /tmp/tracer
chown root:tracer /tmp/tracer
chmod 2775 /tmp/tracer
newgrp tracer

cd /home/ubuntu/tracer-client

echo "Installing cargo-nextest..."
source /root/.cargo/env
cargo install --locked cargo-nextest

echo "Building Tracer..."
cd /root/tracer-client
cargo build --release

echo "Installing Tracer binary..."
sudo cp /root/tracer-client/target/release/tracer_cli /usr/local/bin/tracer
sudo chmod +x /usr/local/bin/tracer

echo "Setting Up test Environment $(date)"
cd /root/tracer-client

echo "Running deployment script for nextflow..."
./deployments/scripts/setup_nextflow_test_env.sh

echo "Installation completed successfully"

echo "Setting up Tracer configuration..."
mkdir -p /root/.config/tracer/

cat <<EOL > /root/.config/tracer/tracer.toml
polling_interval_ms = 1500
service_url = "https://app.tracer.bio/api"
api_key = "$API_KEY"
aws_role_arn = "$ROLE_ARN"
process_polling_interval_ms = 25
batch_submission_interval_ms = 5000
new_run_pause_ms = 600000
file_size_not_changing_period_ms = 60000
process_metrics_send_interval_ms = 10000
aws_region = "us-east-2"
database_secrets_arn = "arn:aws:secretsmanager:us-east-1:395261708130:secret:rds!cluster-cd690a09-953c-42e9-9d9f-1ed0b434d226-M0wZYA"
database_host = "tracer-cluster-production.cluster-ro-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432"
database_name = "tracer_db"
grafana_workspace_url = "https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com"
EOL

echo "Configuration file created at /root/.config/tracer/tracer.toml"
echo "Tracer setup completed successfully at $(date)"