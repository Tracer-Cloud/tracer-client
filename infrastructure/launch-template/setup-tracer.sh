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

# Write tracer.toml config file
# cat <<EOL > /root/.config/tracer/tracer.toml
# polling_interval_ms = 1500
# service_url = "https://app.tracer.bio/api"
# api_key = "${api_key}"
# aws_role_arn = "${role_arn}"
# process_polling_interval_ms = 25
# batch_submission_interval_ms = 5000
# new_run_pause_ms = 600000
# file_size_not_changing_period_ms = 60000
# process_metrics_send_interval_ms = 10000
# aws_region = "us-east-2"
# database_secrets_arn = "arn:aws:secretsmanager:us-east-1:395261708130:secret:rds!cluster-cd690a09-953c-42e9-9d9f-1ed0b434d226-M0wZYA"
# database_host = "tracer-cluster-production.cluster-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432"
# database_name = "tracer_db"
# grafana_workspace_url = "https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com"
# sentry_dsn = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248"
# EOL

# echo "Configuration file created at /root/.config/tracer/tracer.toml"

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