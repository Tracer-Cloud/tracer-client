use crate::constants::{SANDBOX_URL_DEV, SANDBOX_URL_PROD};
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::daemon::structs::PipelineMetadata;
use crate::utils::cli::BoxFormatter;
use crate::utils::env::is_development_environment;
use crate::utils::Version;

pub async fn info(api_client: &DaemonClient, json: bool) {
    if !DaemonServer::is_running() {
        let display = InfoDisplay::new(80, json);
        display.print_error();
        return;
    }
    let pipeline_data = match api_client.send_info_request().await {
        Ok(pipeline_data) => pipeline_data,
        Err(_) => {
            return;
        }
    };
    let display = InfoDisplay::new(180, json);
    display.print(pipeline_data);
}

pub struct InfoDisplay {
    width: usize,
    json: bool,
}

impl InfoDisplay {
    pub const PREVIEW_LIMIT: Option<(usize, usize)> = Some((180, 20));

    pub fn new(width: usize, json: bool) -> Self {
        Self { width, json }
    }

    pub fn print(&self, pipeline: PipelineMetadata) {
        if self.json {
            self.print_json(pipeline);
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);

        self.format_output(&mut formatter, pipeline);

        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    fn print_json(&self, pipeline: PipelineMetadata) {
        let mut json = serde_json::json!({});

        // Tracer CLI info
        json["tracer_cli"] = serde_json::json!({
            "version": Version::current().to_string(),
        });

        // OpenTelemetry status
        if let Some(otel_status) = &pipeline.opentelemetry_status {
            json["opentelemetry"] = serde_json::json!({
                "enabled": otel_status.enabled,
                "version": otel_status.version,
                "pid": otel_status.pid,
                "endpoint": otel_status.endpoint,
            });
        } else {
            json["opentelemetry"] = serde_json::json!({
                "enabled": false,
                "status": "unknown"
            });
        }

        // Pipeline info
        json["pipeline"] = serde_json::json!({
            "status": format!("Running for {}", pipeline.formatted_runtime()),
            "name": &pipeline.name,
            "environment": pipeline.tags.environment.as_deref().unwrap_or("Not set"),
            "environment_type": pipeline.tags.environment_type.as_deref().unwrap_or("Not detected"),
            "instance_type": pipeline.tags.instance_type.as_deref().unwrap_or("Not detected"),
            "user": pipeline.tags.user_id.as_deref().unwrap_or("Not set"),
            "organization": pipeline.tags.organization_slug,
            "email": pipeline.tags.email.as_deref().unwrap_or("Not set"),
            "stage": pipeline.stage(),
        });

        if let Some(run_snapshot) = &pipeline.run_snapshot {
            json["run"] = serde_json::json!({
                "status": format!("Running for {}", run_snapshot.formatted_runtime()),
                "name": &run_snapshot.name,
                "id": &run_snapshot.id,
                "monitored_processes": run_snapshot.process_count(),
                "monitored_tasks": run_snapshot.tasks_count(),
                "dashboard_url": run_snapshot.get_run_url(pipeline.tags.organization_slug.clone(), pipeline.name.clone(), is_development_environment()),
            });

            if run_snapshot.process_count() > 0 {
                json["run"]["processes"] = serde_json::json!(run_snapshot.processes_preview(None));
            }
            if run_snapshot.tasks_count() > 0 {
                json["run"]["tasks"] = serde_json::json!(run_snapshot.tasks_preview(None));
            }

            if let Some(summary) = &run_snapshot.cost_summary {
                json["cost_estimation"] = serde_json::json!({
                    "estimated_cost_since_start": format!("{:.4}", summary.get_estimated_total(run_snapshot.start_time)),
                    "detected_ec2_instance_type": summary.instance_type,
                });
            }
        } else {
            json["run"] = serde_json::json!({
                "status": "No run found",
            });
        }

        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }

    fn format_output(&self, formatter: &mut BoxFormatter, pipeline: PipelineMetadata) {
        formatter.add_header("Tracer CLI");
        formatter.add_empty_line();
        formatter.add_field("Version", &Version::current().to_string(), "bold");
        formatter.add_field("Stage", pipeline.stage(), "bold");

        formatter.add_empty_line();
        formatter.add_section_header("Pipeline details");
        formatter.add_empty_line();
        formatter.add_status_field(
            "Pipeline status",
            format!("Running for {}", pipeline.formatted_runtime()).as_str(),
            "active",
        );

        let pipeline_environment = pipeline.tags.environment.as_deref().unwrap_or("Not set");
        let pipeline_user = pipeline.tags.user_id.as_deref().unwrap();

        let user_email = pipeline.tags.email.as_deref().unwrap_or("Not set");
        let user_organization = pipeline.tags.organization_slug.as_str();

        formatter.add_field("Pipeline name", &pipeline.name, "cyan");
        formatter.add_field("Environment", pipeline_environment, "yellow");
        formatter.add_field("User", pipeline_user, "magenta");
        formatter.add_field("Organization", user_organization, "magenta");
        formatter.add_field("Email", user_email, "magenta");

        if let Some(otel_status) = &pipeline.opentelemetry_status {
            let status_text = if otel_status.enabled {
                format!(
                    "Running (PID: {})",
                    otel_status.pid.map_or("N/A".to_string(), |p| p.to_string())
                )
            } else {
                "Stopped".to_string()
            };
            let status_color = if otel_status.enabled {
                "active"
            } else {
                "inactive"
            };
            formatter.add_status_field("Logging", &status_text, status_color);
        } else {
            formatter.add_status_field("Logging", "Unknown", "inactive");
        }

        formatter.add_empty_line();
        if let Some(run_snapshot) = &pipeline.run_snapshot {
            formatter.add_section_header("Run details");
            formatter.add_empty_line();
            formatter.add_status_field(
                "Run status",
                format!("Running for {}", run_snapshot.formatted_runtime()).as_str(),
                "active",
            );
            formatter.add_hyperlink(
                "Dashboard URL",
                &run_snapshot.get_run_url(
                    pipeline.tags.organization_slug.clone(),
                    pipeline.name.clone(),
                    pipeline.is_dev,
                ),
            );
            formatter.add_field("Run name", &run_snapshot.name, "cyan");
            formatter.add_field("Run ID", &run_snapshot.id, "white");
            let monitored_processes = run_snapshot.process_count();
            let monitored_tasks = run_snapshot.tasks_count();
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
                    &format!(
                        "  $ {:.4}",
                        summary.get_estimated_total(run_snapshot.start_time)
                    ),
                    "yellow",
                );
                formatter.add_field("Instance Type (EC2)", &summary.instance_type, "white");
                formatter.add_empty_line();
            }
        } else {
            formatter.add_section_header("Run details");
            formatter.add_empty_line();
            formatter.add_field("Run status", "No run found", "red");
            formatter.add_empty_line();
        }

        formatter.add_section_header("Resources");
        formatter.add_empty_line();
        formatter.add_field("Dashboard", self.get_sandbox_url(&pipeline), "blue");
        formatter.add_field("Docs", "https://www.tracer.cloud/docs", "blue");
        formatter.add_field("Support", "support@tracer.cloud", "blue");
        formatter.add_empty_line();
    }

    pub fn print_error(&self) {
        if self.json {
            println!("{}", serde_json::json!({"error": "Daemon not started"}));
            return;
        }
        let mut formatter = BoxFormatter::new(self.width);
        formatter.add_header("Tracer CLI");
        formatter.add_empty_line();
        formatter.add_field("Version", &Version::current().to_string(), "bold");
        formatter.add_field(
            "Stage",
            if is_development_environment() {
                "dev"
            } else {
                "prod"
            },
            "bold",
        );
        formatter.add_empty_line();
        formatter.add_section_header("Pipeline details");
        formatter.add_empty_line();
        formatter.add_status_field("Pipeline status", "Not started", "inactive");
        formatter.add_empty_line();
        formatter.add_section_header("Next steps");
        formatter.add_empty_line();
        formatter.add_field("Get started", "tracer login && tracer init", "cyan");
        formatter.add_field("Dashboard", self.get_sandbox_url_from_env(), "blue");
        formatter.add_field("Docs", "https://www.tracer.cloud/docs", "blue");
        formatter.add_field("Support", "support@tracer.cloud", "blue");
        formatter.add_empty_line();
        formatter.add_footer();
        println!("{}", formatter.get_output());
    }

    pub fn get_sandbox_url(&self, pipeline: &PipelineMetadata) -> &str {
        if pipeline.is_dev {
            SANDBOX_URL_DEV
        } else {
            SANDBOX_URL_PROD
        }
    }

    pub fn get_sandbox_url_from_env(&self) -> &str {
        if is_development_environment() {
            SANDBOX_URL_DEV
        } else {
            SANDBOX_URL_PROD
        }
    }
}
