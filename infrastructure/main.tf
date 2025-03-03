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

# -----------------------------------------------------------
# IAM Role for EC2 Instance Connect (For SSH Access Only)
# -----------------------------------------------------------
resource "aws_iam_role" "ec2_instance_connect" {
  name = "EC2InstanceConnectRole"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "ec2_instance_connect" {
  name = "EC2InstanceConnectPolicy"
  role = aws_iam_role.ec2_instance_connect.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ec2-instance-connect:SendSSHPublicKey"
        ],
        Resource = "*"
      }
    ]
  })
}

resource "aws_iam_instance_profile" "ec2_instance_connect" {
  name = "EC2InstanceConnectProfile"
  role = aws_iam_role.ec2_instance_connect.name
}

# -----------------------------------------------------------
# IAM Role for General AWS Access (EC2, S3, Pricing, etc.)
# -----------------------------------------------------------
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

# Attach IAM Policy to the Correct Role
resource "aws_iam_role_policy_attachment" "ec2_general_access_attachment" {
  role       = aws_iam_role.ec2_general_access_role.name
  policy_arn = aws_iam_policy.ec2_general_access.arn
}

# IAM Instance Profile for EC2 General Access Role
resource "aws_iam_instance_profile" "ec2_general_access_profile" {
  name = "EC2GeneralAccessProfile"
  role = aws_iam_role.ec2_general_access_role.name
}

# -----------------------------------------------------------
# Security Group for SSH & EC2 Instance Connect
# -----------------------------------------------------------
data "aws_vpc" "default" {
  default = true
}

resource "aws_security_group" "rust_server_sg" {
  name_prefix = "rust-sg-"
  description = "Allow SSH and EC2 Instance Connect"
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
    Name = "rust-server-security-group"
  }
}

# -----------------------------------------------------------
# EC2 Instance with Full AWS Access
# -----------------------------------------------------------
resource "aws_instance" "rust_server" {
  ami                    = "ami-06f77771310e204b7" # Ubuntu 22.04 LTS
  instance_type          = "c7g.12xlarge"
  key_name               = "rapid-ec2-v1"
  iam_instance_profile   = aws_iam_instance_profile.ec2_general_access_profile.name
  vpc_security_group_ids = [aws_security_group.rust_server_sg.id]

  metadata_options {
    http_tokens                 = "optional"
    http_put_response_hop_limit = 1
    http_endpoint               = "enabled"
  }

  root_block_device {
    volume_size = 50
    volume_type = "gp3"
  }

  tags = {
    Name = "Rust-EC2-Instance"
  }


  # Use templatefile() instead of file()
  user_data = templatefile("${path.module}/script-install-deps.sh", {
    role_arn = aws_iam_role.tracer_client_service_role.arn
    api_key  = var.api_key
  })



}

# -----------------------------------------------------------
# IAM Policy for Full AWS Access (EC2, S3, Pricing API, etc.)
# -----------------------------------------------------------
resource "aws_iam_policy" "ec2_general_access" {
  name        = "EC2GeneralAccessPolicy"
  description = "Allows EC2 instance to interact with AWS services"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      # Allow full access to EC2 metadata and describe operations
      {
        Effect = "Allow"
        Action = [
          "ec2:*" # Allow all EC2 actions
        ]
        Resource = "*"
      },
      # Allow read/write access to S3
      {
        Effect = "Allow"
        Action = [
          "s3:*" # Allow all S3 actions
        ]
        Resource = "*"
      },
      # Allow access to Pricing API
      {
        Effect = "Allow"
        Action = [
          "pricing:*" # Allow pricing actions
        ]
        Resource = "*"
      },
      # Allow CloudWatch Logs Access
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents",
          "logs:DescribeLogStreams",
          "logs:GetLogEvents",
          "logs:DescribeLogGroups"
        ]
        Resource = "*"
      },
      # Allow access to Secrets Manager
      {
        Effect = "Allow"
        Action = [
          "secretsmanager:GetSecretValue",
          "secretsmanager:ListSecrets"
        ]
        Resource = "*"
      },
      # Allow access to AWS Systems Manager (SSM) for Parameter Store and Session Manager
      {
        Effect = "Allow"
        Action = [
          "ssm:GetParameter",
          "ssm:GetParameters",
          "ssm:DescribeParameters",
          "ssm:GetParameterHistory",
          "ssm:GetParametersByPath",
          "ssm:StartSession",
          "ssm:TerminateSession",
          "ssm:DescribeSessions",
          "ssm:DescribeInstanceInformation"
        ]
        Resource = "*"
      },
      # Allow STS Assume Role (useful for cross-account access)
      {
        Effect = "Allow"
        Action = [
          "sts:AssumeRole"
        ]
        Resource = "*"
      }
    ]
  })
}

# create a service role ec2 instance can assume.

resource "aws_iam_role" "tracer_client_service_role" {
  name        = "TracerClientServiceRole"
  description = "Allows EC2 instance to interact with AWS services, including full S3 access"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          AWS = aws_iam_role.ec2_general_access_role.arn #Allow ec2_general_access to assume this role
        }
        Action = "sts:AssumeRole"
      }
    ]
  })
}

resource "aws_iam_role_policy" "s3_full_access" {
  name = "S3FullAccessPolicy"
  role = aws_iam_role.tracer_client_service_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:*", "s3-object-lambda:*"]
        Resource = "*"
      }
    ]
  })
}


# # Create an AMI from the running instance ------>>>>> At the moment we don't have benefits from the AMI
# resource "aws_ami_from_instance" "rust_server_ami" {
#   name               = "rust-server-ami-${formatdate("YYYYMMDD-hhmmss", timestamp())}"
#   source_instance_id = aws_instance.rust_server.id
#   description        = "AMI with Rust and dependencies preinstalled"

#   lifecycle {
#     create_before_destroy = true
#   }

#   tags = {
#     Name = "RustServerAMI"
#   }
# }
