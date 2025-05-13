


output "aurora_host" {
  value = "${aws_rds_cluster.aurora_cluster.endpoint}:${aws_rds_cluster.aurora_cluster.port}"
}

output "aurora_secret_arn" {
  value = aws_rds_cluster.aurora_cluster.master_user_secret[0].secret_arn
}
