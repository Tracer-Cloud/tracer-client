variable "region" {
  description = "The AWS region to deploy resources"
  default     = "us-east-1"
}

variable "api_key" {
  description = "API key for Tracer service"
  type        = string
  sensitive   = true # Prevents logging in Terraform outputs
  default     = "your-api-key"
}

variable "instance_type" {
  description = "Instance type for EC2"
  default     = "c7g.12xlarge" # "c5.12xlarge" 
}

variable "key_name" {
  description = "EC2 Key Pair name for SSH access"
  default     = "rapid-ec2-v1"
}


variable "root_volume_size" {
  description = "Size of the root volume in GB"
  default     = 50
}

variable "root_volume_type" {
  description = "Type of the root volume"
  default     = "gp3"
}


# variable "ubuntu_ami_id" {
#   description = "AMI id of ubuntu image"
#   default     = "ami-0e1bed4f06a3b463d" # x86-64 ami
# }

