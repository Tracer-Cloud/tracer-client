#!/bin/bash
set -e

# Accept role ARN and API key from terraform
ROLE_ARN="${role_arn}"
API_KEY="${api_key}"

cat <<EOF > /tmp/env_vars.sh
export GITHUB_USERNAME="${github_username}"
export GITHUB_TOKEN="${github_token}"
EOF

chmod 600 /tmp/env_vars.sh  # Secure the file
echo "Using ROLE_ARN: $ROLE_ARN"
echo "Using API_KEY: $API_KEY"

LOG_FILE="/root/install_log.txt"
exec > >(tee -a "$LOG_FILE") 2>&1

echo "Starting installation at $(date)"

# Fix any broken dpkg processes
dpkg --configure -a || true
apt clean
apt autoclean

# Update package lists
apt update -y

# Install all required dependencies
apt install -y \
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
    ca-certificates

# Install Docker
echo "Installing Docker..."
curl -fsSL https://get.docker.com -o get-docker.sh
groupadd docker 2>/dev/null || true
usermod -aG docker root
bash get-docker.sh

# No need for newgrp, it doesn't persist in scripts

# OpenSSL env for aarch64
ARCH=$(uname -m)
if [ "$ARCH" = "aarch64" ]; then
    echo "Setting OpenSSL environment variables for ARM (aarch64)..."
    echo 'export OPENSSL_DIR=/usr/lib/aarch64-linux-gnu' >> /etc/profile.d/openssl.sh
    echo 'export OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu' >> /etc/profile.d/openssl.sh
    echo 'export OPENSSL_INCLUDE_DIR=/usr/include' >> /etc/profile.d/openssl.sh
    echo 'export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig' >> /etc/profile.d/openssl.sh
    chmod +x /etc/profile.d/openssl.sh
    source /etc/profile.d/openssl.sh
else
    echo "Skipping OpenSSL config for non-aarch64 architecture: $ARCH"
fi

# Install Rust for root
echo "Installing Rust..."
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source /root/.cargo/env
rustc --version

# Add Rust to system-wide PATH
echo 'export PATH=/root/.cargo/bin:$PATH' > /etc/profile.d/rust.sh
chmod +x /etc/profile.d/rust.sh

# Install GitHub CLI
echo "Installing GitHub CLI..."
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list
apt update -y
apt install -y gh
gh --version || echo "Error: GitHub CLI not installed correctly" >> "$LOG_FILE"

# Clone Tracer repo
echo "Cloning Tracer repository..."
if [ ! -d "/root/tracer-client" ]; then
    git clone https://github.com/Tracer-Cloud/tracer-client.git /root/tracer-client
else
    echo "Tracer repo already exists, pulling latest changes..."
    cd /root/tracer-client && git pull
fi

# Setup /tmp/tracer dir
echo "Setting up /tmp/tracer directory and permissions..."
groupadd -f tracer
usermod -aG tracer root
mkdir -p /tmp/tracer
chown root:tracer /tmp/tracer
chmod 2775 /tmp/tracer

# Install cargo-nextest
echo "Installing cargo-nextest..."
source /root/.cargo/env
cargo install --locked cargo-nextest

# Build the Tracer binary
echo "Building Tracer..."
cd /root/tracer-client
source /root/.cargo/env
cargo build --release

# Install the binary
echo "Installing Tracer binary..."
cp /root/tracer-client/target/release/tracer_cli /usr/local/bin/tracer
chmod +x /usr/local/bin/tracer

# Setup test env
echo "Running test environment setup..."
cat <<EOF > /tmp/env_vars.sh
export GITHUB_USERNAME="${GITHUB_USERNAME}"
export GITHUB_TOKEN="${GITHUB_TOKEN}"
EOF
chmod 600 /tmp/env_vars.sh

source /tmp/env_vars.sh
cd /root/tracer-client
./deployments/scripts/setup_nextflow_test_env.sh

# Write the config file
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