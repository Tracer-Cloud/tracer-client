use crate::daemon::client::DaemonClient;
use crate::daemon::structs::PipelineData;
use crate::utils::cli::BoxFormatter;
use crate::utils::Version;

pub async fn info(api_client: &DaemonClient, json: bool) {
    let pipeline_data = match api_client.send_info_request().await {
        Ok(pipeline_data) => pipeline_data,
        Err(_) => {
            let mut display = InfoDisplay::new(80, json);
            display.print_daemon_error();
            return;
        }
    };
    let display = InfoDisplay::new(150, json);
    display.print(pipeline_data);
}

pub struct InfoDisplay {
    width: usize,
    json: bool,
}

impl InfoDisplay {
    pub const PREVIEW_LIMIT: Option<(usize, usize)> = Some((120, 20));

    pub fn new(width: usize, json: bool) -> Self {
        Self { width, json }
    }

    pub fn print(&self, pipeline: PipelineData) {
        if self.json {
            self.print_json(pipeline);
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);

        self.format_status_pipeline(&mut formatter, pipeline);

        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn print_json(&self, pipeline: PipelineData) {
        let mut json = serde_json::json!({});
        if let Some(run_snapshot) = &pipeline.run_snapshot {
            json["tracer_status"] = serde_json::json!({
                "status": format!("Running for {}", &run_snapshot.formatted_runtime()).as_str(),
                "version": Version::current().to_string(),
            });
            json["pipeline"] = serde_json::json!({
                "name": &pipeline.name,
                "environment": pipeline.tags.environment.as_deref().unwrap_or("Not set"),
                "user": pipeline.tags.user_id.as_deref().unwrap(),
            });
            json["run"] = serde_json::json!({
                "name": &run_snapshot.name,
                "id": &run_snapshot.id,
                "monitored_processes": &run_snapshot.process_count(),
                "monitored_tasks": &run_snapshot.tasks_count(),
            });
            if run_snapshot.process_count() > 0 {
                json["run"]["processes"] = serde_json::json!(run_snapshot.processes_preview(None));
            }
            if run_snapshot.tasks_count() > 0 {
                json["run"]["tasks"] = serde_json::json!(run_snapshot.tasks_preview(None));
            }
            json["run"]["dashboard_url"] =
                serde_json::json!(run_snapshot.get_run_url(pipeline.name));
            json["run"]["stage"] = serde_json::json!(if pipeline.is_dev { "dev" } else { "prod" });
            if let Some(summary) = &run_snapshot.cost_summary {
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

    fn format_status(&self, formatter: &mut BoxFormatter, pipeline: &PipelineData) {
        let run = pipeline.run_snapshot.as_ref().unwrap();
        formatter.add_header("Tracer status");
        formatter.add_empty_line();
        formatter.add_status_field(
            "Status",
            format!("Running for {}", run.formatted_runtime()).as_str(),
            "active",
        );
        formatter.add_field("Version", &Version::current().to_string(), "bold");
        formatter.add_hyperlink("Dashboard", &run.get_run_url(pipeline.name.clone()));
        formatter.add_field("Stage", pipeline.stage(), "bold");

        formatter.add_empty_line();
    }

    fn format_status_pipeline(&self, formatter: &mut BoxFormatter, pipeline: PipelineData) {
        if pipeline.run_snapshot.is_none() {
            formatter.add_section_header("Pipeline details");
            formatter.add_empty_line();
            formatter.add_status_field("Status", "No run found", "inactive");
            formatter.add_empty_line();
            return;
        }
        let run_snapshot = pipeline.run_snapshot.as_ref().unwrap();

        self.format_status(formatter, &pipeline);

        formatter.add_section_header("Pipeline details");
        formatter.add_empty_line();

        let pipeline_environment = pipeline.tags.environment.as_deref().unwrap_or("Not set");
        let pipeline_user = pipeline.tags.user_id.as_deref().unwrap();

        let monitored_processes = run_snapshot.process_count();
        let monitored_tasks = run_snapshot.tasks_count();

        formatter.add_field("Pipeline name", &pipeline.name, "cyan");
        formatter.add_field("Environment", pipeline_environment, "yellow");
        formatter.add_field("User", pipeline_user, "magenta");

        formatter.add_empty_line();
        formatter.add_section_header("Run details");
        formatter.add_empty_line();

        formatter.add_field("Run name", &run_snapshot.name, "cyan");
        formatter.add_field("Run ID", &run_snapshot.id, "white");
        formatter.add_field(
            "Monitored processes",
            &format!("{} processes", monitored_processes),
            "yellow",
        );
        if monitored_processes > 0 {
            formatter.add_multiline_field(
                "Processes preview",
                &run_snapshot.processes_preview(Self::PREVIEW_LIMIT),
                "white",
            );
        }
        formatter.add_field(
            "Monitored tasks",
            &format!("{} tasks", monitored_tasks),
            "yellow",
        );
        if monitored_tasks > 0 {
            formatter.add_multiline_field(
                "Tasks preview",
                &run_snapshot.tasks_preview(Self::PREVIEW_LIMIT),
                "white",
            );
        }
        formatter.add_empty_line();

        if let Some(summary) = &run_snapshot.cost_summary {
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

    pub fn print_daemon_error(&mut self) {
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
