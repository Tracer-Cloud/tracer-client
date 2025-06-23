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

variable "perm_key" {
  description = "Permission Key for accessing the instance"
  type        = string
  default     = "tracer-from-ami"
}

variable "ami_id" {
  description = "AMI ID for the launch template. Should be pre-configured with bioinformatics tools"
  type        = string
  default     = "ami-0dcbd591823292f6a" # Latest AMI with bioinformatics tools
}

variable "instance_type" {
  description = "EC2 instance type for the launch template"
  type        = string
  default     = "c6g.2xlarge"
}
