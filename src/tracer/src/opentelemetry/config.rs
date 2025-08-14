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
        crate::info_message!(
            "Generating OpenTelemetry config for run_id: {}",
            self.run_id
        );

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
        TRACER_WORK_DIR
            .init()
            .with_context(|| "Failed to initialize tracer work directory")?;

        let config_content = self.generate_config()?;
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");

        if config_path.exists() {
            fs::remove_file(&config_path).with_context(|| {
                format!("Failed to remove existing config file at {:?}", config_path)
            })?;
        }

        fs::write(&config_path, &config_content).with_context(|| {
            format!("Failed to write OpenTelemetry config to {:?}", config_path)
        })?;

        if !config_path.exists() {
            return Err(anyhow::anyhow!(
                "Configuration file was not created at {:?}",
                config_path
            ));
        }

        crate::success_message!(
            "OpenTelemetry configuration file created/updated successfully at {:?}",
            config_path
        );

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

        if !content.contains(&self.run_id) {
            return Err(anyhow::anyhow!(
                "Configuration file does not contain expected run_id: {}",
                self.run_id
            ));
        }

        if !content.contains(&self.pipeline_name) {
            return Err(anyhow::anyhow!(
                "Configuration file does not contain expected pipeline name: {}",
                self.pipeline_name
            ));
        }

        if let Some(ref run_name) = self.run_name {
            if !content.contains(run_name) {
                return Err(anyhow::anyhow!(
                    "Configuration file does not contain expected run_name: {}",
                    run_name
                ));
            }
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

        crate::info_message!(
            "Configuration file: {:?} ({} bytes)",
            config_path,
            content.len()
        );

        Ok(())
    }

    pub fn force_recreate_config(&self) -> Result<PathBuf> {
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");

        if config_path.exists() {
            fs::remove_file(&config_path).with_context(|| {
                format!("Failed to remove existing config file at {:?}", config_path)
            })?;
        }

        self.save_config()
    }
}
