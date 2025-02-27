output "security_group_id" {
  value = aws_security_group.tracer_rust_server_sg.id
}



output "iam_instance_profile_name" {
  value = aws_iam_instance_profile.ec2_general_access_profile.name
}


output "service_role_arn" {
  value = aws_iam_role.tracer_client_service_role.arn
}
