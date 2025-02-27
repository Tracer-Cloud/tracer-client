provider "aws" {
  region  = var.region
  profile = "default"
}

# -----------------------------------------------------------
# Fetch the latest Ubuntu 22.04 LTS AMI
# -----------------------------------------------------------
data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"] # Canonical
  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-arm64-server-*"]
  }
}

# -------------------------------------
# Resouce for generating random string.
# -------------------------------------
resource "random_string" "suffix" {
  length  = 8
  special = false
  upper   = false
}

# ---------------------------------------------
# IAM Role for EC2 Instance Connect (SSH Access)
# ---------------------------------------------
resource "aws_iam_role" "ec2_instance_connect" {
  name = "EC2InstanceConnectRole-${random_string.suffix.result}"

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
  name = "EC2InstanceConnectPolicy-${random_string.suffix.result}"
  role = aws_iam_role.ec2_instance_connect.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["ec2-instance-connect:SendSSHPublicKey"]
      Resource = "*"
    }]
  })
}

resource "aws_iam_instance_profile" "ec2_instance_connect" {
  name = "EC2InstanceConnectProfile"
  role = aws_iam_role.ec2_instance_connect.name
}

# ---------------------------------------------
# IAM Role for General AWS Access
# ---------------------------------------------
resource "aws_iam_role" "ec2_general_access_role" {
  name = "EC2GeneralAccessRole-${random_string.suffix.result}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy_attachment" "ec2_general_access_attachment" {
  role       = aws_iam_role.ec2_general_access_role.name
  policy_arn = aws_iam_policy.ec2_general_access.arn
}

resource "aws_iam_instance_profile" "ec2_general_access_profile" {
  name = "EC2GeneralAccessProfile-${random_string.suffix.result}"
  role = aws_iam_role.ec2_general_access_role.name
}

# ---------------------------------------------
# Security Group (Allow SSH)
# ---------------------------------------------
data "aws_vpc" "default" {
  default = true
}

resource "aws_security_group" "rust_server_sg" {
  name_prefix = "rust-sg-"
  description = "Allow SSH and EC2 Instance Connect"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"] # WARNING: Open SSH access!
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "rust-server-security-group-${timestamp()}"
  }
}

# ---------------------------------------------
# EC2 Instance Deployment
# ---------------------------------------------
resource "aws_instance" "rust_server" {
  ami                    = data.aws_ami.ubuntu.id
  instance_type          = var.instance_type
  key_name               = var.key_name
  iam_instance_profile   = aws_iam_instance_profile.ec2_general_access_profile.name
  vpc_security_group_ids = [aws_security_group.rust_server_sg.id]

  metadata_options {
    http_tokens                 = "optional"
    http_put_response_hop_limit = 1
    http_endpoint               = "enabled"
  }

  root_block_device {
    volume_size = var.root_volume_size
    volume_type = var.root_volume_type
  }

  tags = {
    Name = "Rust-EC2-Instance-${random_string.suffix.result}"
  }

  user_data = templatefile("${path.module}/script-install-deps.sh", {
    role_arn = aws_iam_role.tracer_client_service_role.arn
    api_key  = var.api_key
  })
}

# ---------------------------------------------
# IAM Policy for AWS Service Access
# ---------------------------------------------
resource "aws_iam_policy" "ec2_general_access" {
  name        = "EC2GeneralAccessPolicy-${random_string.suffix.result}"
  description = "Allows EC2 instance to interact with AWS services"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["ec2:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["pricing:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["secretsmanager:GetSecretValue", "secretsmanager:ListSecrets"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["ssm:GetParameter", "ssm:DescribeInstanceInformation", "ssm:StartSession"]
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

# ---------------------------------------------
# IAM Role for S3 Full Access
# ---------------------------------------------
resource "aws_iam_role" "tracer_client_service_role" {
  name        = "TracerClientServiceRole-${random_string.suffix.result}"
  description = "Allows EC2 instance to interact with AWS services, including full S3 access"

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
  name = "S3FullAccessPolicy-${random_string.suffix.result}"
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
