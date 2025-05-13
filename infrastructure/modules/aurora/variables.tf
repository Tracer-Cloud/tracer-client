variable "region" {
  description = "AWS region"
  default     = "us-east-1"
}

variable "subnet_ids" {
  description = "List of subnet IDs for the Aurora cluster"
  type        = list(string)
}

variable "security_group_ids" {
  description = "List of subnet IDs for the Aurora cluster"
  type        = list(string)
  default     = []
}

variable "aurora_db_instance_class" {
  description = "List of subnet IDs for the Aurora cluster"
  default     = "db.t4g.medium"
}

variable "db_name" {
  description = "Aurora database name"
  default     = "tracer_db"
}

variable "engine_version" {
  description = "Aurora PostgreSQL engine version"
  default     = "16.3"
}
