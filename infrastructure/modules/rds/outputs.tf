output "rds_endpoint" {
  value = aws_db_instance.rds.endpoint
}

output "rds_secret_arn" {
  value = aws_db_instance.rds.master_user_secret[0].secret_arn
}

output "db_instance_id" {
  value = aws_db_instance.rds.id
}
