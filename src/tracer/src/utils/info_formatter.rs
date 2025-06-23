use crate::config::Config;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse};
use crate::process_identification::constants::{LOG_FILE, STDERR_FILE, STDOUT_FILE};
use crate::utils::version::FullVersion;
use anyhow::Result;
use colored::Colorize;
use console::Emoji;
use std::fmt::Write;

const STATUS_ACTIVE: Emoji<'_, '_> = Emoji("🟢 ", "🟢 ");
const STATUS_INACTIVE: Emoji<'_, '_> = Emoji("🔴 ", "🔴 ");
const STATUS_WARNING: Emoji<'_, '_> = Emoji("🟡 ", "🟡 ");
const STATUS_INFO: Emoji<'_, '_> = Emoji("ℹ️ ", "ℹ️ ");

pub struct InfoFormatter {
    output: String,
    width: usize,
}

impl InfoFormatter {
    pub fn new(width: usize) -> Self {
        Self {
            output: String::new(),
            width,
        }
    }

    pub fn add_header(&mut self, title: &str) -> Result<()> {
        writeln!(
            &mut self.output,
            "\n┌{:─^width$}┐",
            format!(" {} ", title),
            width = self.width - 2
        )?;
        Ok(())
    }

    pub fn add_footer(&mut self) -> Result<()> {
        writeln!(
            &mut self.output,
            "└{:─^width$}┘",
            "",
            width = self.width - 2
        )?;
        Ok(())
    }

    pub fn add_section_header(&mut self, title: &str) -> Result<()> {
        writeln!(
            &mut self.output,
            "├{:─^width$}┤",
            format!(" {} ", title),
            width = self.width - 2
        )?;
        Ok(())
    }

    pub fn add_field(&mut self, label: &str, value: &str, color: &str) -> Result<()> {
        let colored_value = match color {
            "green" => value.green(),
            "yellow" => value.yellow(),
            "cyan" => value.cyan(),
            "magenta" => value.magenta(),
            "blue" => value.blue(),
            "red" => value.red(),
            "bold" => value.bold(),
            _ => value.normal(),
        };

        // Calculate available space for value
        let label_width = 20;
        let padding = 4;
        let max_value_width = self.width - label_width - padding;

        // Format the value with proper truncation
        let formatted_value = if colored_value.len() > max_value_width {
            format!("{}...", &colored_value[..max_value_width - 3])
        } else {
            colored_value.to_string()
        };

        writeln!(
            &mut self.output,
            "│ {:<label_width$} │ {}  ",
            label, formatted_value
        )?;
        Ok(())
    }

    pub fn add_status_field(&mut self, label: &str, value: &str, status: &str) -> Result<()> {
        let (emoji, color) = match status {
            "active" => (STATUS_ACTIVE, "green"),
            "inactive" => (STATUS_INACTIVE, "red"),
            "warning" => (STATUS_WARNING, "yellow"),
            _ => (STATUS_INFO, "blue"),
        };

        writeln!(
            &mut self.output,
            "│ {:<20} │ {} {}  ",
            label,
            emoji,
            value.color(color)
        )?;
        Ok(())
    }

    pub fn add_empty_line(&mut self) -> Result<()> {
        writeln!(&mut self.output, "│{:width$}│", "", width = self.width - 2)?;
        Ok(())
    }

    pub fn get_output(&self) -> &str {
        &self.output
    }

    pub fn print_error_state(&mut self) -> Result<()> {
        self.add_header("TRACER CLI STATUS")?;
        self.add_empty_line()?;
        self.add_status_field("Daemon Status", "Not Started", "inactive")?;
        self.add_field("Version", &FullVersion::current().to_string(), "bold")?;
        self.add_empty_line()?;
        self.add_section_header("NEXT STEPS")?;
        self.add_empty_line()?;
        self.add_field("Interactive Setup", "tracer init", "bold")?;
        self.add_field("Visualize Data", "https://sandbox.tracer.app", "blue")?;
        self.add_field(
            "Documentation",
            "https://github.com/Tracer-Cloud/tracer-client",
            "blue",
        )?;
        self.add_field("Support", "support@tracer.cloud", "blue")?;
        self.add_empty_line()?;
        self.add_footer()?;
        Ok(())
    }

    pub fn print_daemon_status(&mut self) -> Result<()> {
        self.add_section_header("DAEMON STATUS")?;
        self.add_empty_line()?;
        self.add_status_field("Status", "Running", "active")?;
        self.add_field("Version", &FullVersion::current().to_string(), "bold")?;
        self.add_empty_line()?;
        Ok(())
    }

    pub fn print_pipeline_info(
        &mut self,
        inner: &InnerInfoResponse,
        info: &InfoResponse,
    ) -> Result<()> {
        self.add_section_header("RUN DETAILS")?;
        self.add_empty_line()?;

        // Pipeline section
        self.add_field("Pipeline Name", &inner.pipeline_name, "bold")?;
        self.add_field(
            "Pipeline Type",
            inner.tags.pipeline_type.as_deref().unwrap_or("Not Set"),
            "cyan",
        )?;
        self.add_field(
            "Environment",
            inner.tags.environment.as_deref().unwrap_or("Not Set"),
            "yellow",
        )?;
        self.add_field(
            "User",
            inner.tags.user_operator.as_deref().unwrap_or("Not Set"),
            "magenta",
        )?;

        // Run section
        self.add_field("Run Name", &inner.run_name, "bold")?;
        self.add_field("Run ID", &inner.run_id, "cyan")?;
        self.add_field("Runtime", &inner.formatted_runtime(), "yellow")?;
        self.add_field(
            "Monitored Processes",
            &format!(
                "{}: {}",
                info.watched_processes_count.to_string().bold(),
                info.watched_processes_preview().cyan()
            ),
            "normal",
        )?;
        self.add_empty_line()?;
        Ok(())
    }

    pub fn print_config_and_logs(&mut self, config: &Config) -> Result<()> {
        self.add_section_header("CONFIGURATION & LOGS")?;
        self.add_empty_line()?;

        let grafana_url = if config.grafana_workspace_url.is_empty() {
            "Not configured".to_string()
        } else {
            config.grafana_workspace_url.clone()
        };

        self.add_field("Grafana Workspace", &format!("{} ", grafana_url), "blue")?;
        self.add_field(
            "Process Polling",
            &format!("{} ms ", config.process_polling_interval_ms),
            "yellow",
        )?;
        self.add_field(
            "Batch Submission",
            &format!("{} ms ", config.batch_submission_interval_ms),
            "yellow",
        )?;
        self.add_field("Standard Output", &format!("{} ", STDOUT_FILE), "cyan")?;
        self.add_field("Standard Error", &format!("{} ", STDERR_FILE), "cyan")?;
        self.add_field("Log File", &format!("{} ", LOG_FILE), "cyan")?;
        Ok(())
    }
}
