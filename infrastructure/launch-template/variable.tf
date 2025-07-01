variable "region" {
  description = "The AWS region to deploy resources"
  default     = "us-east-1"
}

# Do not log sensitive data 
variable "api_key" {
  description = "API key for Tracer service"
  type        = string
  sensitive   = true
  default     = "your-secret-api-key"
}

variable "perm_key" {
  description = "Permission Key for accessing the instance"
  type        = string
  default     = "tracer-from-ami"
}

variable "ami_id" {
  description = "AMI ID for the launch template. Should be pre-configured with bioinformatics tools"
  type        = string
  default     = "ami-066807aa78bd9a0ce" # ami-066807aa78bd9a0ce - Ubuntu 22.04 aarch64 
}

variable "instance_type" {
  description = "EC2 instance type for the launch template"
  type        = string
  default     = "c6g.2xlarge"
}
