

output "workspace_id" {
  value = aws_grafana_workspace.tracer_workspace.id
}

output "workspace_url" {
  value = "https://${aws_grafana_workspace.tracer_workspace.endpoint}"
}
