terraform {
  backend "s3" {
    bucket         = "tracer-cloud-terraform-state"
    key            = "launch_template/terraform.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "tf-launch-template-state"
  }
}

data "aws_vpc" "default" {
  default = true
}

variable "region" {
  description = "The AWS region to deploy resources"
  default     = "us-east-1"
}

variable "api_key" {
  description = "API key for Tracer service"
  type        = string
  sensitive   = true # This prevents it from being logged in Terraform outputs
  default     = "your-secret-api-key"
}

provider "aws" {
  region  = var.region
  profile = "default"
}


variable "perm_key" {
  description = "Permission Key for accessing the instance"
  type        = string
  default     = "tracer-from-ami"
}


# ---------------------------
# Security Group for EC2
# ---------------------------
resource "aws_security_group" "tracer_sg" {
  name_prefix = "tracer-sg-"
  description = "Allow SSH and outbound traffic"
  vpc_id      = data.aws_vpc.default.id

  # Allow SSH from any IP (Restrict in production)
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Allow all outbound traffic
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "tracer-security-group"
  }
}

# ---------------------------
# IAM Role for EC2 Instance
# ---------------------------
resource "aws_iam_role" "ec2_general_access_role" {
  name = "EC2GeneralAccessRole"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_policy" "ec2_general_access" {
  name        = "EC2GeneralAccessPolicy"
  description = "Allows EC2 instance to interact with AWS services"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["ec2:*", "s3:*", "pricing:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["logs:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["secretsmanager:GetSecretValue", "secretsmanager:ListSecrets"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["ssm:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["sts:AssumeRole"]
        Resource = "*"
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "ec2_attach" {
  role       = aws_iam_role.ec2_general_access_role.name
  policy_arn = aws_iam_policy.ec2_general_access.arn
}

resource "aws_iam_instance_profile" "ec2_instance_profile" {
  name = "EC2InstanceProfile"
  role = aws_iam_role.ec2_general_access_role.name
}

# ---------------------------
# Service Role EC2 Can Assume
# ---------------------------
resource "aws_iam_role" "tracer_client_service_role" {
  name = "TracerClientServiceRole"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { AWS = aws_iam_role.ec2_general_access_role.arn }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "s3_full_access" {
  name = "S3FullAccessPolicy"
  role = aws_iam_role.tracer_client_service_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["s3:*", "s3-object-lambda:*"]
      Resource = "*"
    }]
  })
}

# ---------------------------
# EC2 Launch Template
# ---------------------------
resource "aws_launch_template" "tracer_demo" {
  name_prefix   = "tracer-demo"
  image_id      = "ami-08963412c7663a4b8"
  instance_type = "c6g.large"

  key_name = var.perm_key

  iam_instance_profile {
    name = aws_iam_instance_profile.ec2_instance_profile.name
  }

  network_interfaces {
    associate_public_ip_address = true
    security_groups             = [aws_security_group.tracer_sg.id]
  }

  user_data = base64encode(templatefile("${path.module}/setup-tracer.sh", {
    role_arn = aws_iam_role.tracer_client_service_role.arn
    api_key  = var.api_key
  }))

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name = "tracer-instance"
    }
  }
}
