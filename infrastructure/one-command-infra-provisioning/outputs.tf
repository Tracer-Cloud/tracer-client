
output "grafana_url" {
  value = module.grafana_workspace.workspace_url
}

output "ec2_public_ip" {
  value = aws_instance.rust_server.public_ip
}


output "rds_endpoint" {
  value = module.rds.rds_endpoint
}
