# ---------------------------
# Security Group for EC2
# ---------------------------
resource "aws_security_group" "tracer_rust_server_sg" {
  name_prefix = "rust-sg-${var.name_suffix}"
  description = "Allow SSH and outbound traffic"
  vpc_id      = var.vpc_id


  ingress {
    from_port       = 22
    to_port         = 22
    protocol        = "tcp"
    cidr_blocks     = ["0.0.0.0/0"] # Restrict in production
    security_groups = var.security_group_ids

  }

  # Allow PostgreSQL (RDS)
  ingress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"] # Change to allow specific IPs or security groups in production
  }

  # Allow all outbound traffic
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "rust-server-security-group-${var.name_suffix}"
  }
}

# ---------------------------
# IAM Role for EC2 Instance
# ---------------------------
resource "aws_iam_role" "ec2_instance_connect" {
  name = "EC2InstanceConnectRole-${var.name_suffix}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}
resource "aws_iam_role" "ec2_general_access_role" {
  name = "EC2GeneralAccessRole-${var.name_suffix}"

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
  name        = "EC2GeneralAccessPolicy-${var.name_suffix}"
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
        "Action" : [
          "secretsmanager:*"
        ],
        "Effect" : "Allow",
        "Resource" : "*"
      },
      {
        Effect   = "Allow"
        Action   = ["sts:AssumeRole"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["pricing:GetProducts"]
        Resource = "*"
      },
    ]
  })
}

resource "aws_iam_role_policy_attachment" "ec2_general_access_attachment" {
  role       = aws_iam_role.ec2_general_access_role.name
  policy_arn = aws_iam_policy.ec2_general_access.arn
}

resource "aws_iam_instance_profile" "ec2_general_access_profile" {
  name = "EC2GeneralAccessProfile-${var.name_suffix}"
  role = aws_iam_role.ec2_general_access_role.name
}

resource "aws_iam_role_policy" "ec2_instance_connect_policy" {
  name = "EC2InstanceConnectPolicy-${var.name_suffix}"
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

resource "aws_iam_instance_profile" "ec2_instance_connect_profile" {
  name = "EC2InstanceConnectProfile-${var.name_suffix}"
  role = aws_iam_role.ec2_instance_connect.name
}



# ---------------------------------------------
# IAM Role for S3 Full Access
# ---------------------------------------------
resource "aws_iam_role" "tracer_client_service_role" {
  name        = "TracerClientServiceRole-${var.name_suffix}"
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

resource "aws_iam_role_policy" "service_access_policy" {
  name = "TracerServiceAccessPolicy-${var.name_suffix}"
  role = aws_iam_role.tracer_client_service_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:*", "s3-object-lambda:*"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["secretsmanager:GetSecretValue", "secretsmanager:DescribeSecret", "secretsmanager:ListSecrets"]
        Resource = "**arn:aws:secretsmanager:*:*:secret:rds*"
      },
      {
        Effect   = "Allow"
        Action   = ["pricing:GetProducts"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["logs:CreateLogGroup", "logs:CreateLogStream", "logs:PutLogEvents"]
        Resource = "*"
      },
      {
        Effect   = "Allow"
        Action   = ["ssm:GetParameter", "ssm:DescribeInstanceInformation", "ssm:StartSession"]
        Resource = "*"
      },
    ]
  })
}

