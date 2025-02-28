provider "aws" {
  region  = var.region
  profile = "default"
}


resource "aws_security_group" "db_sg" {
  name        = "rds_sg"
  description = "Security group for RDS access"

  dynamic "ingress" {
    for_each = length(var.security_group_ids) > 0 ? [] : [1]
    content {
      from_port   = 5432
      to_port     = 5432
      protocol    = "tcp"
      cidr_blocks = ["0.0.0.0/0"] # Default if no security group is passed
    }
  }

  dynamic "ingress" {
    for_each = var.security_group_ids
    content {
      from_port       = 5432
      to_port         = 5432
      protocol        = "tcp"
      security_groups = [ingress.value]
    }
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


resource "aws_db_instance" "rds" {
  identifier                  = "tracer-rds-${random_string.suffix.result}"
  engine                      = "postgres"
  instance_class              = var.db_instance_class
  allocated_storage           = 10
  max_allocated_storage       = 100
  username                    = var.db_username
  manage_master_user_password = true
  vpc_security_group_ids      = [aws_security_group.db_sg.id]
  skip_final_snapshot         = true

}


