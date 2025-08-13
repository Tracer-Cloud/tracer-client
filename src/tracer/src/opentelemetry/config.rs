use crate::constants::OTEL_FORWARD_ENDPOINT;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct OtelConfig {
    pub user_id: String,
    pub pipeline_name: String,
    pub run_name: Option<String>,
    pub run_id: String,
    pub environment_variables: HashMap<String, String>,
}

impl OtelConfig {
    pub fn new(
        user_id: String,
        pipeline_name: String,
        run_name: Option<String>,
        run_id: String,
    ) -> Self {
        Self {
            user_id,
            pipeline_name,
            run_name,
            run_id,
            environment_variables: HashMap::new(),
        }
    }

    pub fn with_environment_variables(
        user_id: String,
        pipeline_name: String,
        run_name: Option<String>,
        run_id: String,
        environment_variables: HashMap<String, String>,
    ) -> Self {
        Self {
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
        for (key, value) in &self.environment_variables {
            std::env::set_var(key, value);
            crate::info_message!(
                "Set environment variable: {}={}",
                key,
                if key.to_lowercase().contains("key")
                    || key.to_lowercase().contains("secret")
                    || key.to_lowercase().contains("password")
                {
                    "***"
                } else {
                    value
                }
            );
        }

        Ok(())
    }

    pub fn generate_config(&self) -> Result<String> {
        crate::info_message!("Generating OpenTelemetry config with values:");
        crate::info_message!("  user_id: {}", self.user_id);
        crate::info_message!("  pipeline_name: {}", self.pipeline_name);
        crate::info_message!("  run_name: {:?}", self.run_name);
        crate::info_message!("  run_id: {}", self.run_id);
        crate::info_message!("  endpoint: {}", OTEL_FORWARD_ENDPOINT);

        let template_content = include_str!("otel-config-template.yaml");

        let config = template_content
            .replace("{{user_id}}", &self.user_id)
            .replace("{{pipeline_name}}", &self.pipeline_name)
            .replace(
                "{{run_name}}",
                self.run_name.as_deref().unwrap_or("unknown"),
            )
            .replace("{{run_id}}", &self.run_id)
            .replace("{{otel_endpoint}}", OTEL_FORWARD_ENDPOINT);

        Ok(config)
    }

    pub fn save_config(&self) -> Result<PathBuf> {
        crate::info_message!("Starting save_config method...");

        // Ensure the work directory is initialized
        crate::info_message!("Initializing tracer work directory...");
        TRACER_WORK_DIR
            .init()
            .with_context(|| "Failed to initialize tracer work directory")?;
        crate::info_message!("Tracer work directory initialized successfully");

        crate::info_message!("Generating configuration content...");
        let config_content = self.generate_config()?;
        crate::info_message!(
            "Configuration content generated successfully ({} bytes)",
            config_content.len()
        );

        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");
        crate::info_message!("Configuration will be saved to: {:?}", config_path);

        // Always create a fresh configuration file with current run data
        crate::info_message!(
            "Creating fresh OpenTelemetry configuration for run_id: {}",
            self.run_id
        );
        crate::info_message!("Configuration path: {:?}", config_path);
        crate::info_message!("Pipeline: {}", self.pipeline_name);
        crate::info_message!("Run name: {:?}", self.run_name);

        // Remove existing config file if it exists to ensure fresh content
        if config_path.exists() {
            crate::info_message!("Removing existing configuration file to create fresh one");
            fs::remove_file(&config_path).with_context(|| {
                format!("Failed to remove existing config file at {:?}", config_path)
            })?;
            crate::info_message!("Existing configuration file removed successfully");
        } else {
            crate::info_message!("No existing configuration file found, creating new one");
        }

        crate::info_message!("Writing configuration content to file...");
        fs::write(&config_path, &config_content).with_context(|| {
            format!("Failed to write OpenTelemetry config to {:?}", config_path)
        })?;
        crate::info_message!("Configuration content written to file successfully");

        // Verify the file was created
        if config_path.exists() {
            let metadata = fs::metadata(&config_path)?;
            crate::info_message!(
                "Configuration file created successfully, size: {} bytes",
                metadata.len()
            );
        } else {
            return Err(anyhow::anyhow!(
                "Configuration file was not created at {:?}",
                config_path
            ));
        }

        crate::success_message!(
            "OpenTelemetry configuration file created/updated successfully at {:?}",
            config_path
        );
        crate::info_message!("Configuration includes run_id: {}", self.run_id);

        Ok(config_path)
    }

    pub fn verify_config_file(&self) -> Result<()> {
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");

        if !config_path.exists() {
            return Err(anyhow::anyhow!(
                "Configuration file does not exist at {:?}",
                config_path
            ));
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read configuration file at {:?}", config_path))?;

        // Verify that the configuration contains the expected run_id
        if !content.contains(&self.run_id) {
            return Err(anyhow::anyhow!(
                "Configuration file does not contain expected run_id: {}",
                self.run_id
            ));
        }

        // Verify that the configuration contains the expected pipeline name
        if !content.contains(&self.pipeline_name) {
            return Err(anyhow::anyhow!(
                "Configuration file does not contain expected pipeline name: {}",
                self.pipeline_name
            ));
        }

        // Verify that the configuration contains the expected run name if provided
        if let Some(ref run_name) = self.run_name {
            if !content.contains(run_name) {
                return Err(anyhow::anyhow!(
                    "Configuration file does not contain expected run_name: {}",
                    run_name
                ));
            }
        }

        crate::info_message!("Configuration file verification successful");
        crate::info_message!("File contains run_id: {}", self.run_id);
        crate::info_message!("File contains pipeline: {}", self.pipeline_name);
        if let Some(ref run_name) = self.run_name {
            crate::info_message!("File contains run_name: {}", run_name);
        }

        Ok(())
    }

    pub fn show_config_contents(&self) -> Result<()> {
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");

        if !config_path.exists() {
            crate::warning_message!("Configuration file does not exist at {:?}", config_path);
            return Ok(());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read configuration file at {:?}", config_path))?;

        crate::info_message!("Current OpenTelemetry configuration file contents:");
        crate::info_message!("File: {:?}", config_path);
        crate::info_message!("Size: {} bytes", content.len());

        // Show first few lines for debugging
        let lines: Vec<&str> = content.lines().collect();
        let preview_lines = lines.iter().take(20).collect::<Vec<&&str>>();

        for line in preview_lines {
            crate::info_message!("  {}", line);
        }

        if lines.len() > 20 {
            crate::info_message!("  ... (showing first 20 lines of {} total)", lines.len());
        }

        Ok(())
    }

    pub fn force_recreate_config(&self) -> Result<PathBuf> {
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");

        crate::info_message!("Force recreating OpenTelemetry configuration file...");

        // Remove existing file if it exists
        if config_path.exists() {
            crate::info_message!("Removing existing configuration file");
            fs::remove_file(&config_path).with_context(|| {
                format!("Failed to remove existing config file at {:?}", config_path)
            })?;
        }

        // Create fresh configuration
        self.save_config()
    }
}
