#!/bin/bash

# Accept role ARN and API key from terraform
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
database_secrets_arn = "arn:aws:secretsmanager:us-east-1:395261708130:secret:rds!cluster-cd690a09-953c-42e9-9d9f-1ed0b434d226-M0wZYA"
database_host = "tracer-cluster-production.cluster-ro-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432"
database_name = "tracer_db"
grafana_workspace_url = "https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com"
EOL

echo "Configuration file created at /home/ubuntu/.config/tracer/tracer.toml"

# Install the binary
echo "Updating Tracer binary..."
sudo rm /usr/local/bin/tracer
su - ubuntu -c "curl -sSL https://tracer-client.pages.dev/installation-script-development.sh | bash && source ~/.bashrc"
su - ubuntu -c "sudo cp /home/ubuntu/.tracerbio/bin/tracer  /usr/local/bin/"
sudo chown ubuntu:ubuntu /usr/local/bin/tracer
echo "Tracer binary updated successfully"

source ~/.bashrc

su - ubuntu -c "tracer info"

echo "Tracer setup successfully $(date)"