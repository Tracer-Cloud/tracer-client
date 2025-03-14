


resource "aws_security_group" "db_sg" {
  name        = "rds_sg-${random_string.suffix.result}"
  description = "Security group for RDS access"
  vpc_id      = var.vpc_id

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

  # # Allow EC2 security group to access RDS
  # ingress {
  #   from_port       = 5432
  #   to_port         = 5432
  #   protocol        = "tcp"
  #   security_groups = var.security_group_ids # Allow EC2 security group
  # }

  # Allow outbound traffic
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
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
  db_name                     = var.db_name

  # ✅ Attach the subnet group here
  db_subnet_group_name = aws_db_subnet_group.rds_subnet_group.name

}


resource "aws_db_subnet_group" "rds_subnet_group" {
  name       = "rds-subnet-group-${random_string.suffix.result}"
  subnet_ids = var.subnet_ids
}
