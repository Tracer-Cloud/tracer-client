variable "region" {
  description = "The AWS region to deploy resources"
  default     = "us-east-1"
}
variable "db_instance_class" {
  description = "Class of instance to spin up"
  default     = "db.t3.micro"
}

variable "db_username" {
  description = "Username for database"
  default     = "tracer_user"
}

variable "db_name" {
  description = "Username for database"
  default     = "tracer_db"
}

variable "security_group_ids" {
  description = "List of security group IDs that should be allowed to access the database"
  type        = list(string)
  # default     = [] # Empty list by default
}

variable "vpc_id" {
  type = string
}

variable "subnet_ids" {
  description = "Set of subnet ids"
  type        = list(string)
  default     = [] # Empty list by default
}
