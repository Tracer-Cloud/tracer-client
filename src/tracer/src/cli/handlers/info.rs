use crate::daemon::client::DaemonClient;
use crate::daemon::structs::{InfoResponse, InnerInfoResponse};
use crate::utils::cli::BoxFormatter;
use crate::utils::Version;

pub async fn info(api_client: &DaemonClient, json: bool) {
    let info = match api_client.send_info_request().await {
        Ok(info) => info,
        Err(e) => {
            let mut display = InfoDisplay::new(80, json);
            tracing::error!("Error getting info response: {e}");
            display.print_error();
            return;
        }
    };
    let display = InfoDisplay::new(150, json);
    display.print(info);
}

pub struct InfoDisplay {
    width: usize,
    json: bool,
}

impl InfoDisplay {
    pub const PREVIEW_LENGTH: Option<usize> = Some(10);

    pub fn new(width: usize, json: bool) -> Self {
        Self { width, json }
    }

    pub fn print(&self, info: InfoResponse) {
        if self.json {
            self.print_json(info);
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);

        self.format_status_pipeline_info(&mut formatter, info);

        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn print_json(&self, info: InfoResponse) {
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
                "user": inner.tags.user_operator.as_deref().unwrap_or("Not set")
            });
            json["run"] = serde_json::json!({
                "name": &inner.run_name,
                "id": &inner.run_id,
                "monitored_processes": &info.process_count(),
                "monitored_tasks": &info.tasks_count(),
            });
            if info.process_count() > 0 {
                json["run"]["processes"] = serde_json::json!(info.processes_preview(None));
            }
            if info.tasks_count() > 0 {
                json["run"]["tasks"] = serde_json::json!(info.tasks_preview(None));
            }
            json["run"]["dashboard_url"] = serde_json::json!(inner.get_run_url());
            json["run"]["stage"] = serde_json::json!(inner.stage);

            if let Some(summary) = &inner.cost_summary {
                json["run"]["estimated_cost_since_start"] =
                    serde_json::json!(format!("{:.4}", summary.estimated_total));
                json["run"]["detected_ec2_instance_type"] =
                    serde_json::json!(summary.instance_type);
            }
        } else {
            //todo Can we even print info active if no pipeline is running? Should inner even be an option?
            json["pipeline_info"] = serde_json::json!({
                "status": "No run found",
            });
            return;
        }
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }

    fn format_status(&self, formatter: &mut BoxFormatter, inner: &InnerInfoResponse) {
        formatter.add_header("Tracer status");
        formatter.add_empty_line();
        formatter.add_status_field(
            "Status",
            format!("Running for {}", inner.formatted_runtime()).as_str(),
            "active",
        );
        formatter.add_field("Version", &Version::current().to_string(), "bold");
        formatter.add_hyperlink("Dashboard", &inner.get_run_url());
        formatter.add_field("Stage", &inner.stage, "bold");

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

        self.format_status(formatter, inner);

        formatter.add_section_header("Pipeline details");
        formatter.add_empty_line();

        let pipeline_type = inner.tags.pipeline_type.as_deref().unwrap_or("Not set");
        let pipeline_environment = inner.tags.environment.as_deref().unwrap_or("Not set");
        let pipeline_user = inner.tags.user_operator.as_deref().unwrap_or("Not set");

        let monitored_processes = info.process_count();
        let monitored_tasks = info.tasks_count();

        formatter.add_field("Pipeline name", &inner.pipeline_name, "cyan");
        formatter.add_field("Pipeline type", pipeline_type, "white");
        formatter.add_field("Environment", pipeline_environment, "yellow");
        formatter.add_field("User", pipeline_user, "magenta");

        formatter.add_empty_line();
        formatter.add_section_header("Run details");
        formatter.add_empty_line();

        formatter.add_field("Run name", &inner.run_name, "cyan");
        formatter.add_field("Run ID", &inner.run_id, "white");
        formatter.add_field(
            "Monitored processes",
            &format!("{} processes", monitored_processes),
            "yellow",
        );
        if monitored_processes > 0 {
            formatter.add_field(
                "Processes preview",
                &info.processes_preview(Self::PREVIEW_LENGTH),
                "white",
            );
        }
        formatter.add_field(
            "Monitored tasks",
            &format!("{} tasks", monitored_tasks),
            "yellow",
        );
        if monitored_tasks > 0 {
            formatter.add_field(
                "Tasks preview",
                &info.tasks_preview(Self::PREVIEW_LENGTH),
                "white",
            );
        }
        formatter.add_empty_line();

        if let Some(summary) = &inner.cost_summary {
            formatter.add_section_header("Cost estimation");
            formatter.add_empty_line();
            formatter.add_field(
                "Total since start",
                &format!("  $ {:.4}", summary.estimated_total),
                "yellow",
            );
            formatter.add_field("Instance Type (EC2)", &summary.instance_type, "white");
            formatter.add_empty_line();
        }
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
        formatter.add_hyperlink("Sandbox", "https://sandbox.tracer.cloud");
        formatter.add_hyperlink(
            "Documentation",
            "https://github.com/Tracer-Cloud/tracer-client",
        );
        formatter.add_field("Support", "support@tracer.cloud", "blue");
        formatter.add_empty_line();
        formatter.add_footer();
        println!("{}", formatter.get_output());
    }
}
