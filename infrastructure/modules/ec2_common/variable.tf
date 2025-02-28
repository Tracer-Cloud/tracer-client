variable "region" {
  description = "The AWS region"
  default     = "us-east-1"
}

variable "name_suffix" {
  description = "Prefix for resource names"
  type        = string
  default     = "test"
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}
