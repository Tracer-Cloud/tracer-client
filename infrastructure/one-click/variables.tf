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

variable "aws_account_id" {
  default = "us-east-1"
}

variable "db_username" {
  description = "Username for database"
  default     = "tracer_user"
}

variable "db_name" {
  description = "Username for database"
  default     = "tracer_db"
}

variable "instance_type" {
  description = "Instance type for EC2"
  default     = "c7g.12xlarge" #"c5.12xlarge"
}

variable "root_volume_size" {
  description = "Size of the root volume in GB"
  default     = 50
}

variable "root_volume_type" {
  description = "Type of the root volume"
  default     = "gp3"
}
