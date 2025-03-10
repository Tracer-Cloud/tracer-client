variable "region" {
  default = "us-east-1"
}

variable "security_group_ids" {
  description = "List of private subnet IDs"
  type        = list(string)
}

variable "subnet_ids" {
  description = "List of public subnet IDs"
  type        = list(string)
}


variable "grafana_api_key" {
  description = "API key to connect to grafana"
  sensitive   = false
}
