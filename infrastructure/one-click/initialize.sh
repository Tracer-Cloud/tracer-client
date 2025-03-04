#!/bin/bash

# Accept role ARN and API key from terraform
#FIXME: launch template should update tracer for the current main because image ami could be months old
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
database_secrets_arn = "${database_secret_manager_arn}"
database_host = "${db_endpoint}"
database_name = "${database_name}"
EOL

echo "Configuration file created at /home/ubuntu/.config/tracer/tracer.toml"

source ~/.bashrc

# Build the Tracer binary
echo "Updating Tracer..."
su - ubuntu -c "source /home/ubuntu/.cargo/env && cd /home/ubuntu/tracer-client && git fetch && git checkout -f ENG-92/one-click-deploy && cargo build --release"

# Install the binary
echo "Updating Tracer binary..."
sudo rm /usr/local/bin/tracer
sudo cp /home/ubuntu/tracer-client/target/release/tracer /usr/local/bin/
echo "Tracer binary updated successfully"

su - ubuntu -c "tracer init --pipeline-name one-click"

echo "Tracer setup successfully $(date)"