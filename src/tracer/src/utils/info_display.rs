use crate::config::Config;
use crate::constants::{GRAFANA_WORKSPACE_DASHBOARD, TRACER_SANDBOX_URL};
use crate::daemon::structs::InfoResponse;
use crate::process_identification::constants::{LOG_FILE, STDERR_FILE, STDOUT_FILE};
use crate::utils::cli::BoxFormatter;
use crate::utils::FullVersion;

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

        formatter.add_header("TRACER INFO");
        formatter.add_empty_line();

        self.format_status(&mut formatter);
        self.format_pipeline_info(&mut formatter, info);

        self.format_config_and_logs(&mut formatter, config);
        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn print_json(&self, info: InfoResponse, config: &Config) {
        let mut json = serde_json::json!({});
        json["daemon_status"] = serde_json::json!({
            "status": "Running",
            "version": FullVersion::current().to_string(),
        });
        if let Some(inner) = &info.inner {
            json["pipeline"] = serde_json::json!({
                "name": &inner.pipeline_name,
                "type": inner.tags.pipeline_type.as_deref().unwrap_or("Not Set"),
                "environment": inner.tags.environment.as_deref().unwrap_or("Not Set"),
                "user": inner.tags.user_operator.as_deref().unwrap_or("Not Set"),
                "dashboard_url": inner.get_pipeline_url(),
            });
            json["run"] = serde_json::json!({
                "name": &inner.run_name,
                "id": &inner.run_id,
                "run": &inner.start_time,
                "runtime": &inner.formatted_runtime(),
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
                "status": "No Run Found",
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
        json["links"] = serde_json::json!({
            "sandbox": TRACER_SANDBOX_URL,
            "workspace_dashboard": GRAFANA_WORKSPACE_DASHBOARD,
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }
    fn format_status(&self, formatter: &mut BoxFormatter) {
        formatter.add_section_header("DAEMON STATUS");
        formatter.add_empty_line();
        formatter.add_status_field("Status", "Running", "active");
        formatter.add_field("Version", &FullVersion::current().to_string(), "bold");
        formatter.add_empty_line();
    }

    fn format_pipeline_info(&self, formatter: &mut BoxFormatter, info: InfoResponse) {
        if info.inner.is_none() {
            //todo Can we even print info active if no pipeline is running? Should inner even be an option?
            formatter.add_section_header("PIPELINE & RUN DETAILS");
            formatter.add_empty_line();
            formatter.add_status_field("Status", "No Run Found", "inactive");
            formatter.add_empty_line();
            return;
        }
        let inner = info.inner.as_ref().unwrap();

        formatter.add_section_header("PIPELINE & RUN DETAILS");
        formatter.add_empty_line();

        let pipeline_type = inner.tags.pipeline_type.as_deref().unwrap_or("Not Set");
        let pipeline_environment = inner.tags.environment.as_deref().unwrap_or("Not Set");
        let pipeline_user = inner.tags.user_operator.as_deref().unwrap_or("Not Set");

        let run_runtime = &inner.formatted_runtime();
        let monitored_processes = &info.watched_processes_count;

        formatter.add_field("Pipeline Name", &inner.pipeline_name, "cyan");
        formatter.add_field("Pipeline Type", pipeline_type, "white");
        formatter.add_field("Environment", pipeline_environment, "yellow");
        formatter.add_field("User", pipeline_user, "magenta");
        formatter.add_hyperlink("Pipeline Dashboard", &inner.get_pipeline_url(), "View");

        formatter.add_empty_line();

        formatter.add_field("Run Name", &inner.run_name, "cyan");
        formatter.add_field("Run ID", &inner.run_id, "white");
        formatter.add_field("Runtime", run_runtime, "green");
        formatter.add_field(
            "Monitored Processes",
            &format!("{} processes", monitored_processes),
            "yellow",
        );
        if !info.watched_processes_preview().is_empty() {
            formatter.add_field(
                "Process Preview",
                &info.watched_processes_preview(),
                "white",
            );
        }

        formatter.add_hyperlink("Run Dashboard", &inner.get_run_url(), "View");

        formatter.add_empty_line();
    }

    pub fn print_error(&mut self) {
        if self.json {
            println!("{}", serde_json::json!({"error": "Daemon not started"}));
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);
        formatter.add_header("TRACER CLI STATUS");
        formatter.add_empty_line();
        formatter.add_status_field("Daemon Status", "Not Started", "inactive");
        formatter.add_field("Version", &FullVersion::current().to_string(), "bold");
        formatter.add_empty_line();
        formatter.add_section_header("NEXT STEPS");
        formatter.add_empty_line();
        formatter.add_field("Interactive Setup", "tracer init", "cyan");
        formatter.add_field("Visualize Data", "https://sandbox.tracer.cloud", "blue");
        formatter.add_field(
            "Documentation",
            "https://github.com/Tracer-Cloud/tracer-client",
            "blue",
        );
        formatter.add_field("Support", "support@tracer.cloud", "blue");
        formatter.add_empty_line();
        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn format_config_and_logs(&self, formatter: &mut BoxFormatter, config: &Config) {
        formatter.add_section_header("CONFIGURATION & LOGS");
        formatter.add_empty_line();

        formatter.add_hyperlink("Sandbox Workspace", TRACER_SANDBOX_URL, "View");
        formatter.add_hyperlink("Workspace Dashboard", GRAFANA_WORKSPACE_DASHBOARD, "View");
        formatter.add_field(
            "Polling Interval",
            &format!("{} ms", config.process_polling_interval_ms),
            "yellow",
        );
        formatter.add_field(
            "Batch Interval",
            &format!("{} ms", config.batch_submission_interval_ms),
            "yellow",
        );
        formatter.add_field("Log Files", "Standard Output", "cyan");
        formatter.add_field("", &format!("  {}", STDOUT_FILE), "white");
        formatter.add_field("", &format!("  {}", STDERR_FILE), "white");
        formatter.add_field("", &format!("  {}", LOG_FILE), "white");
    }
}
