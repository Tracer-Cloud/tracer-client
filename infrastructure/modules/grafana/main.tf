terraform {
  required_providers {
    grafana = {
      source  = "grafana/grafana"
      version = "~> 1.40.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

provider "aws" {
  region = var.region
}



# Random suffix for unique naming
resource "random_string" "suffix" {
  length  = 8
  special = false
  upper   = false
}

# Grafana Workspace with VPC Configuration for RDS Access
resource "aws_grafana_workspace" "tracer_workspace" {
  name                     = "tracer-workspace-${random_string.suffix.result}"
  account_access_type      = "CURRENT_ACCOUNT"
  authentication_providers = ["AWS_SSO"]
  permission_type          = "SERVICE_MANAGED"
  role_arn                 = aws_iam_role.assume.arn
  data_sources             = ["AMAZON_OPENSEARCH_SERVICE", "ATHENA", "CLOUDWATCH", "PROMETHEUS"]

  # Enable VPC Configuration to allow access to RDS
  vpc_configuration {
    security_group_ids = var.security_group_ids # Attach security group allowing Grafana to access RDS
    subnet_ids         = var.subnet_ids         # Place Grafana inside the correct subnets
    # vpc_id             = var.vpc_id
  }

  configuration = jsonencode({
    "plugins"         = { "pluginAdminEnabled" = true },
    "unifiedAlerting" = { "enabled" = false }
  })
}

# IAM Role for Grafana Authentication
resource "aws_iam_role" "assume" {
  name = "grafana-assume-${random_string.suffix.result}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "grafana.amazonaws.com" }
    }]
  })
}


resource "grafana_folder" "dashboard_folder" {
  title = "Dashboards"
}


# Import Dashboards
resource "grafana_dashboard" "pipeline_cpu_utilization" {
  folder      = grafana_folder.dashboard_folder.uid
  config_json = file("${path.module}/dashboards/cpu_utilization_by_pipeline.json")
}

resource "grafana_dashboard" "pipelines_preview" {
  folder      = grafana_folder.dashboard_folder.uid
  config_json = file("${path.module}/dashboards/pipelines_preview.json")
}

resource "grafana_dashboard" "pipeline_run_details" {
  folder      = grafana_folder.dashboard_folder.uid
  config_json = file("${path.module}/dashboards/pipeline_run_details.json")
}


