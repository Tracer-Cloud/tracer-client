#!/bin/bash

LOG_FILE="/home/ubuntu/one_click.txt"
exec > >(tee -a "$LOG_FILE") 2>&1  # Log both stdout & stderr

# Accept role ARN and API key from terraform
#FIXME: launch template should update tracer for the current main because image ami could be months old
echo "Setting up Tracer"
# Create the directory for the config file
mkdir -p /home/ubuntu/.config/tracer/

# Create /tmp/tracer directory with proper permissions. Note this is ephemeral and needs to exists on startup
echo "Setting up /tmp/tracer directory and permissions..."
# Idempotently create the tracer group
groupadd -f tracer

# Add users to tracer group
usermod -aG tracer ubuntu
usermod -aG tracer root

# Create tracer directory with sticky group inheritance
mkdir -p /tmp/tracer
chown root:tracer /tmp/tracer
chmod 2775 /tmp/tracer
newgrp tracer




# Write the configuration to tracer.toml
cat <<EOL > /home/ubuntu/.config/tracer/tracer.toml
polling_interval_ms = 1500
service_url = "https://app.tracer.bio/api"
api_key = "${api_key}"
aws_role_arn = "${role_arn}"
process_polling_interval_ms = 25
batch_submission_interval_ms = 10000
new_run_pause_ms = 600000
file_size_not_changing_period_ms = 60000
process_metrics_send_interval_ms = 10000
aws_region = "us-east-2"
database_secrets_arn = "${database_secret_manager_arn}"
database_host = "${db_endpoint}"
database_name = "${database_name}"
grafana_base_url = "${grafana_base_url}"
sentry_dsn = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"
EOL

echo "Configuration file created at /home/ubuntu/.config/tracer/tracer.toml"

source ~/.bashrc

# Install the binary
echo "Updating Tracer binary..."
sudo rm /usr/local/bin/tracer
su - ubuntu -c "curl -sSL https://tracer-client.pages.dev/installation-script-development.sh | bash && source ~/.bashrc"
su - ubuntu -c "sudo cp /home/ubuntu/.tracerbio/bin/tracer  /usr/local/bin/"
sudo chown ubuntu:ubuntu /usr/local/bin/tracer
echo "Tracer binary updated successfully"

# Migrate The database before starting the client

ENCODED_PASS=$(python3 -c "import urllib.parse; print(urllib.parse.quote('${database_password}'))")
su - ubuntu -c "cd /home/ubuntu/tracer-client && git pull origin main && ./migrate.sh postgres://${database_user}:$ENCODED_PASS@${db_endpoint}/${database_name}" 

# start the client
su - ubuntu -c "tracer init --pipeline-name one-click --environment demo --user-operator John --pipeline-type generic"

echo "Script setup ran successfully $(date)"