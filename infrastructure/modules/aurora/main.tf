
resource "random_string" "suffix" {
  length  = 8
  special = false
  upper   = false
}


resource "aws_security_group" "aurora_sg" {
  name        = "aurora_sg-${random_string.suffix.result}"
  description = "Security group for Aurora access"

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



resource "aws_db_subnet_group" "aurora_subnet_group" {
  name       = "aurora-subnet-group-${random_string.suffix.result}"
  subnet_ids = var.subnet_ids
}



resource "aws_rds_cluster" "aurora_cluster" {
  cluster_identifier          = "tracer-aurora-${random_string.suffix.result}"
  engine                      = "aurora-postgresql"
  engine_mode                 = "provisioned"
  engine_version              = var.engine_version
  database_name               = var.db_name
  master_username             = "tracer_user"
  manage_master_user_password = true
  storage_encrypted           = true
  skip_final_snapshot         = true


  # ✅ Attach the subnet and sec group
  db_subnet_group_name   = aws_db_subnet_group.aurora_subnet_group.name
  vpc_security_group_ids = [aws_security_group.aurora_sg.id]

  serverlessv2_scaling_configuration {
    min_capacity = 0
    max_capacity = 2.0
    # seconds_until_auto_pause = 3600
  }
}

resource "aws_rds_cluster_instance" "aurora_instance" {
  cluster_identifier = aws_rds_cluster.aurora_cluster.id
  instance_class     = var.aurora_db_instance_class
  engine             = aws_rds_cluster.aurora_cluster.engine
  engine_version     = aws_rds_cluster.aurora_cluster.engine_version
}

