#!/bin/bash

# Accept role ARN and API key from terraform
echo "Setting up Tracer"
# Create the directory for the config file
mkdir -p /home/ubuntu/.config/tracer/

# Write the configuration to tracer.toml
cat <<EOL > /home/ubuntu/.config/tracer/tracer.toml
polling_interval_ms = 1500
service_url = "https://app.tracer.bio/api"
api_key = "${api_key}"
aws_role_arn = "${role_arn}"
process_polling_interval_ms = 5
batch_submission_interval_ms = 10000
new_run_pause_ms = 600000
file_size_not_changing_period_ms = 60000
process_metrics_send_interval_ms = 10000
aws_region = "us-east-2"
db_url = "postgres://postgres:tracer-test@tracer-database.cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432/postgres"
EOL

echo "Configuration file created at /home/ubuntu/.config/tracer/tracer.toml"

# Install the binary
echo "Updating Tracer binary..."
sudo rm /usr/local/bin/tracer
su - ubuntu -c "source /home/ubuntu/.cargo/env && cd /home/ubuntu/tracer-client && git pull origin main && cargo build --release"
sudo cp /home/ubuntu/tracer-client/target/release/tracer /usr/local/bin/
echo "Tracer binary updated successfully"

source ~/.bashrc

su - ubuntu -c "tracer init --pipeline-name launch-template"

echo "Tracer setup successfully $(date)"