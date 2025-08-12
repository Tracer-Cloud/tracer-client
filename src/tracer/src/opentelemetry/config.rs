use crate::constants::OPENSEARCH_ENDPOINT;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct OtelConfig {
    pub opensearch_api_key: String,
    pub user_id: String,
    pub pipeline_name: String,
    pub run_name: Option<String>,
    pub run_id: String,
    pub environment_variables: HashMap<String, String>,
}

impl OtelConfig {
    pub fn new(
        opensearch_api_key: String,
        user_id: String,
        pipeline_name: String,
        run_name: Option<String>,
        run_id: String,
    ) -> Self {
        Self {
            opensearch_api_key,
            user_id,
            pipeline_name,
            run_name,
            run_id,
            environment_variables: HashMap::new(),
        }
    }

    pub fn with_environment_variables(
        opensearch_api_key: String,
        user_id: String,
        pipeline_name: String,
        run_name: Option<String>,
        run_id: String,
        environment_variables: HashMap<String, String>,
    ) -> Self {
        Self {
            opensearch_api_key,
            user_id,
            pipeline_name,
            run_name,
            run_id,
            environment_variables,
        }
    }

    pub fn add_environment_variable(&mut self, key: String, value: String) {
        self.environment_variables.insert(key, value);
    }

    pub fn set_environment_variables(&self) -> Result<()> {
        // Set the OpenSearch API key
        std::env::set_var("OPENSEARCH_API_KEY", &self.opensearch_api_key);
        crate::info_message!("Set environment variable: OPENSEARCH_API_KEY");
        
        // Set additional environment variables
        for (key, value) in &self.environment_variables {
            std::env::set_var(key, value);
            crate::info_message!("Set environment variable: {}={}", key, if key.to_lowercase().contains("key") || key.to_lowercase().contains("secret") || key.to_lowercase().contains("password") { "***" } else { value });
        }
        
        Ok(())
    }

    pub fn generate_config(&self) -> Result<String> {
        let config = format!(
            r#"receivers:
  filelog:
    include:
      - '**/*.log*'
      - '**/*.out*'
      - '**/*.err*'
      - '**/*.txt*'
      
      - '**/.nextflow.log*'
      - '**/nextflow.log*'
      - '**/.nextflow*.log*'
      - '**/nextflow*.log*'
      - '**/.nextflow/log'
      - '**/work/**/.command.log'
      - '**/work/**/.command.err'
      - '**/work/**/.command.out'
      
      - '**/target/**/*.log*'
      - '**/build/**/*.log*'
      - '**/logs/**/*'
      - '**/log/**/*'
      
      - './*.log*'
      - './*.out*'
      - './*.err*'
      - './*.txt*'
      - './*/*.log*'
      - './*/*.out*'
      - './*/*.err*'
      - './*/*.txt*'

    exclude:
      - /proc/*
      - /proc/*/*
      - /proc/*/*/*
      - /sys/*
      - /sys/*/*
      - /sys/*/*/*
      - /dev/*
      - /dev/*/*
      - /dev/*/*/*
      - /snap/*
      - /snap/*/*
      - /snap/*/*/*
      - /System/*
      - /Library/*
      - /Applications/*
      - '**/node_modules/**'
      - '**/.git/**'
      - '**/target/debug/build/**'
      - '**/target/release/build/**'
      - '**/build/**'
      - '**/vendor/**'
      - '**/.cargo/**'
      - '**/.rustup/**'
      - '**/.local/**'
      - '**/.cache/**'
      - '**/tmp/**'
      - '**/var/tmp/**'
      - '**/.terraform/**'

    start_at: end
    poll_interval: 50ms
    max_log_size: 50MiB
    max_concurrent_files: 4096
    include_file_name: true
    include_file_path: true
    include_file_name_resolved: true
    include_file_path_resolved: true
    fingerprint_size: 2kb
    force_flush_period: 100ms

  hostmetrics:
    collection_interval: 300s
    scrapers:
      memory:

processors:
  batch:
    timeout: 1s
    send_batch_size: 1024

  resource:
    attributes:
      - key: service.name
        value: 'nextflow-pipeline'
        action: insert
      - key: service.version
        value: '1.0.0'
        action: insert
      - key: deployment.environment
        value: 'production'
        action: insert
      - key: host.name
        from_attribute: host.name
        action: insert

  transform:
    log_statements:
      - context: log
        statements:
          - set(attributes["user_id"], "{user_id}")
          - set(attributes["status"], "error") where IsMatch(body, "(?i).*(error|failed|exception|fatal|critical).*")
          - set(attributes["status"], "warning") where IsMatch(body, "(?i).*(warn|warning|caution|deprecated).*")
          - set(attributes["status"], "info") where attributes["status"] == nil
          - set(attributes["log_level"], "INFO")
          - set(attributes["pipeline_name"], "{pipeline_name}")
          - set(attributes["run_name"], "{run_name}")
          - set(attributes["run_id"], "{run_id}")
          - set(attributes["log_file_path"], attributes["log.file.path"]) where attributes["log.file.path"] != nil
          - set(attributes["file_name"], attributes["log.file.name"]) where attributes["log.file.name"] != nil
          - set(attributes["log_message"], body)

exporters:
  awss3:
    s3uploader:
      region: us-east-1
      s3_bucket: tracer-logs
      s3_prefix: tracer_logs
    marshaler: otlp_json

  opensearch:
    http:
      endpoint: '{opensearch_endpoint}'
      headers:
        Authorization: 'Basic {api_key}'
    logs_index: 'logs-{user_id}'
    timeout: 30s
    retry_on_failure:
      enabled: true
      initial_interval: 5s
      max_interval: 30s
      max_elapsed_time: 300s

  debug:
    verbosity: basic
    sampling_initial: 1
    sampling_thereafter: 1000

service:
  pipelines:
    logs:
      receivers: [filelog]
      processors: [resource, transform, batch]
      exporters: [opensearch, awss3, debug]

    metrics:
      receivers: [hostmetrics]
      processors: [resource, batch]
      exporters: [awss3, debug]

  extensions: []

  telemetry:
    logs:
      level: info
"#,
            user_id = self.user_id,
            pipeline_name = self.pipeline_name,
            run_name = self.run_name.as_deref().unwrap_or(""),
            run_id = self.run_id,
            opensearch_endpoint = OPENSEARCH_ENDPOINT,
            api_key = self.opensearch_api_key,
        );

        Ok(config)
    }

    pub fn save_config(&self) -> Result<PathBuf> {
        let config_content = self.generate_config()?;
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");
        
        fs::write(&config_path, config_content)
            .with_context(|| format!("Failed to write OpenTelemetry config to {:?}", config_path))?;
        
        Ok(config_path)
    }
}
