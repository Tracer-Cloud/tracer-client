


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

module "vpc" {
  source = "../modules/vpc"
}

# provider "aws" {
#   region = var.region # Change as needed
# }

data "aws_vpc" "default" {
  default = true
}

data "aws_caller_identity" "current" {}


provider "grafana" {
  url  = module.grafana_workspace.workspace_url
  auth = aws_grafana_workspace_api_key.grafana_api_key.key

}

resource "random_string" "suffix" {
  length  = 8
  special = false
  upper   = false
}


module "grafana_workspace" {
  source             = "../modules/grafana"
  region             = var.region
  subnet_ids         = module.vpc.public_subnet_ids
  grafana_api_key    = aws_grafana_workspace_api_key.grafana_api_key.key
  security_group_ids = [aws_security_group.allow_access.id]
}


# # # # Security Group for Grafana Access to RDS
resource "aws_security_group" "allow_access" {
  name        = "allow_access-${random_string.suffix.result}"
  description = "Allow Grafana to connect to RDS"
  vpc_id      = module.vpc.vpc_id

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"] # Adjust if needed
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

module "rds" {
  source             = "../modules/rds"
  vpc_id             = module.vpc.vpc_id
  db_instance_class  = "db.t3.micro"
  db_name            = "tracer_db"
  region             = var.region
  subnet_ids         = module.vpc.private_subnet_ids
  security_group_ids = [aws_security_group.allow_access.id]
}




# Grafana API Key for Automation
resource "aws_grafana_workspace_api_key" "grafana_api_key" {
  key_name        = "dashboard-import"
  key_role        = "ADMIN"
  seconds_to_live = 2592000 # 30 days (adjust as needed) #9600
  workspace_id    = module.grafana_workspace.workspace_id
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



resource "aws_iam_role" "grafana_rds_role" {
  name = "grafana-rds-role-${random_string.suffix.result}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Principal = {
        Service = "grafana.amazonaws.com"
      }
      Action = "sts:AssumeRole"
    }]
  })
}

# Attach the policy that grants Grafana access to RDS
resource "aws_iam_policy" "grafana_rds_policy" {
  name        = "grafana-rds-policy-${random_string.suffix.result}"
  description = "Allows Grafana to generate IAM auth tokens for RDS"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = "rds-db:connect"
      Resource = "arn:aws:rds-db:${var.region}:${data.aws_caller_identity.current.account_id}:dbuser:${module.rds.db_instance_id}/${var.db_username}"
    }]
  })
}

# Attach policy to IAM role
resource "aws_iam_role_policy_attachment" "grafana_rds_attach" {
  role       = aws_iam_role.grafana_rds_role.name
  policy_arn = aws_iam_policy.grafana_rds_policy.arn
}



data "aws_secretsmanager_secret_version" "db_secret" {
  secret_id = module.rds.rds_secret_arn
}

locals {
  db_credentials = jsondecode(data.aws_secretsmanager_secret_version.db_secret.secret_string)
}

resource "grafana_data_source" "postgres" {
  type = "postgres"

  name          = "tracer-postgres"
  url           = module.rds.rds_endpoint
  database_name = var.db_name
  # username      = local.db_credentials["username"] # Extracted from Secrets Manager
  username   = var.db_username
  is_default = true

  json_data_encoded = jsonencode({
    sslmode      = "require"
    maxOpenConns = 10
    maxIdleConns = 2
    authType     = "credentials"
  })

  secure_json_data_encoded = jsonencode({
    password = local.db_credentials["password"] # Extracted from Secrets Manager
  })
}



# EC2 setup from default ami
#
module "ec2_common" {
  source      = "../modules/ec2_common"
  name_suffix = random_string.suffix.result
  vpc_id      = data.aws_vpc.default.id
}

resource "aws_instance" "rust_server" {
  depends_on             = [module.grafana_workspace, module.rds]
  ami                    = "ami-08963412c7663a4b8"
  instance_type          = var.instance_type
  key_name               = var.perm_key
  iam_instance_profile   = module.ec2_common.iam_instance_profile_name
  vpc_security_group_ids = [module.ec2_common.security_group_id]

  metadata_options {
    http_tokens                 = "optional"
    http_put_response_hop_limit = 1
    http_endpoint               = "enabled"
  }

  # root_block_device {
  #   volume_size = var.root_volume_size
  #   volume_type = var.root_volume_type
  # }

  tags = {
    Name = "Rust-EC2-Instance-${random_string.suffix.result}"
  }

  user_data = templatefile("${path.module}/initialize.sh", {
    role_arn                    = module.ec2_common.service_role_arn
    api_key                     = var.api_key
    database_secret_manager_arn = module.rds.rds_secret_arn
    database_name               = var.db_name
    db_endpoint                 = module.rds.rds_endpoint
  })
}

