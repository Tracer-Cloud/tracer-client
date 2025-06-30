use crate::config::Config;
use crate::daemon::structs::InfoResponse;
use crate::process_identification::constants::{LOG_FILE, STDERR_FILE, STDOUT_FILE};
use crate::utils::cli::BoxFormatter;
use crate::utils::Version;

pub struct InfoDisplay {
    width: usize,
    json: bool,
}

impl InfoDisplay {
    pub fn new(width: usize, json: bool) -> Self {
        Self { width, json }
    }

    pub fn print(&self, info: InfoResponse, config: &Config) {
        if self.json {
            self.print_json(info, config);
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);

        self.format_status_pipeline_info(&mut formatter, info);

        self.format_config_and_logs(&mut formatter, config);
        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn print_json(&self, info: InfoResponse, config: &Config) {
        let mut json = serde_json::json!({});
        if let Some(inner) = &info.inner {
            json["tracer_status"] = serde_json::json!({
                "status": format!("Running for {}", &inner.formatted_runtime()).as_str(),
                "version": Version::current().to_string(),
            });
            json["pipeline"] = serde_json::json!({
                "name": &inner.pipeline_name,
                "type": inner.tags.pipeline_type.as_deref().unwrap_or("Not set"),
                "environment": inner.tags.environment.as_deref().unwrap_or("Not set"),
                "user": inner.tags.user_operator.as_deref().unwrap_or("Not set"),
                "dashboard_url": inner.get_pipeline_url(),
            });
            json["run"] = serde_json::json!({
                "name": &inner.run_name,
                "id": &inner.run_id,
                "monitored_processes": &info.watched_processes_count,
            });
            if !info.watched_processes_preview().is_empty() {
                json["run"]["preview_processes"] =
                    serde_json::json!(info.watched_processes_preview());
            }
            json["run"]["dashboard_url"] = serde_json::json!(inner.get_run_url());
        } else {
            //todo Can we even print info active if no pipeline is running? Should inner even be an option?
            json["pipeline_info"] = serde_json::json!({
                "status": "No run found",
            });
            return;
        }

        json["config"] = serde_json::json!({
            "polling_interval": config.process_polling_interval_ms,
            "batch_interval": config.batch_submission_interval_ms,
        });
        json["log_files"] = serde_json::json!({
            "stdout": STDOUT_FILE,
            "stderr": STDERR_FILE,
            "log": LOG_FILE,
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }
    fn format_status(&self, formatter: &mut BoxFormatter, runtime: &String) {
        formatter.add_header("Tracer status");
        formatter.add_empty_line();
        formatter.add_status_field(
            "Status",
            format!("Running for {}", runtime).as_str(),
            "active",
        );
        formatter.add_field("Version", &Version::current().to_string(), "bold");
        formatter.add_empty_line();
    }

    fn format_status_pipeline_info(&self, formatter: &mut BoxFormatter, info: InfoResponse) {
        if info.inner.is_none() {
            //todo Can we even print info active if no pipeline is running? Should inner even be an option?
            formatter.add_section_header("Pipeline & run details");
            formatter.add_empty_line();
            formatter.add_status_field("Status", "No run found", "inactive");
            formatter.add_empty_line();
            return;
        }
        let inner = info.inner.as_ref().unwrap();

        self.format_status(formatter, &inner.formatted_runtime());

        formatter.add_section_header("Pipeline details");
        formatter.add_empty_line();

        let pipeline_type = inner.tags.pipeline_type.as_deref().unwrap_or("Not set");
        let pipeline_environment = inner.tags.environment.as_deref().unwrap_or("Not set");
        let pipeline_user = inner.tags.user_operator.as_deref().unwrap_or("Not set");

        let monitored_processes = &info.watched_processes_count;

        formatter.add_field("Pipeline name", &inner.pipeline_name, "cyan");
        formatter.add_field("Pipeline type", pipeline_type, "white");
        formatter.add_field("Environment", pipeline_environment, "yellow");
        formatter.add_field("User", pipeline_user, "magenta");
        formatter.add_hyperlink("Open dashboard ↗️", "Dashboard", &inner.get_pipeline_url());

        formatter.add_empty_line();
        formatter.add_section_header("Run details");

        formatter.add_field("Run name", &inner.run_name, "cyan");
        formatter.add_field("Run ID", &inner.run_id, "white");
        formatter.add_field(
            "Monitored processes",
            &format!("{} processes", monitored_processes),
            "yellow",
        );
        if !info.watched_processes_preview().is_empty() {
            formatter.add_field(
                "Process preview",
                &info.watched_processes_preview(),
                "white",
            );
        }

        formatter.add_hyperlink("Open dashboard ↗️", "Dashboard", &inner.get_run_url());

        formatter.add_empty_line();
    }

    pub fn print_error(&mut self) {
        if self.json {
            println!("{}", serde_json::json!({"error": "Daemon not started"}));
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);
        formatter.add_header("Tracer CLI status");
        formatter.add_empty_line();
        formatter.add_status_field("Daemon status", "Not started", "inactive");
        formatter.add_field("Version", &Version::current().to_string(), "bold");
        formatter.add_empty_line();
        formatter.add_section_header("Next steps");
        formatter.add_empty_line();
        formatter.add_field("Interactive setup", "tracer init", "cyan");
        formatter.add_hyperlink(
            "Visualize data 📈",
            "Sandbox",
            "https://sandbox.tracer.cloud",
        );
        formatter.add_hyperlink(
            "Read docs 📄",
            "Documentation",
            "https://github.com/Tracer-Cloud/tracer-client",
        );
        formatter.add_field("Support", "support@tracer.cloud", "blue");
        formatter.add_empty_line();
        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn format_config_and_logs(&self, formatter: &mut BoxFormatter, config: &Config) {
        formatter.add_section_header("Configuration & logs");
        formatter.add_empty_line();

        formatter.add_field(
            "Polling interval",
            &format!("{} ms", config.process_polling_interval_ms),
            "yellow",
        );
        formatter.add_field(
            "Batch interval",
            &format!("{} ms", config.batch_submission_interval_ms),
            "yellow",
        );
        formatter.add_field("Log files", "Standard output", "cyan");
        formatter.add_field("", &format!("  {}", STDOUT_FILE), "white");
        formatter.add_field("", &format!("  {}", STDERR_FILE), "white");
        formatter.add_field("", &format!("  {}", LOG_FILE), "white");
    }
}
