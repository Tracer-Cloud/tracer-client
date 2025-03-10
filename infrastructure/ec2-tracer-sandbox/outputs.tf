output "instance_id" {
  description = "The ID of the EC2 instance"
  value       = aws_instance.rust_server.id
}

output "instance_public_ip" {
  description = "Public IP of the EC2 instance"
  value       = aws_instance.rust_server.public_ip
}

output "instance_private_ip" {
  description = "Private IP of the EC2 instance"
  value       = aws_instance.rust_server.private_ip
}
