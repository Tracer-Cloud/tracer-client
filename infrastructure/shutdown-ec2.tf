# -----------------------------------------------------------
# IAM Role for EC2 to Stop Itself via SSM
# -----------------------------------------------------------
resource "aws_iam_role" "ec2_ssm_role" {
  name = "EC2SSMShutdownRole"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "ec2.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_policy" "ssm_shutdown_policy" {
  name = "SSMShutdownPolicy"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ssm:SendCommand",
          "ssm:GetCommandInvocation"
        ]
        Resource = "*"
      },
      {
        Effect = "Allow"
        Action = [
          "ec2:StopInstances"
        ]
        Resource = "*"
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "ssm_shutdown_attachment" {
  role       = aws_iam_role.ec2_ssm_role.name
  policy_arn = aws_iam_policy.ssm_shutdown_policy.arn
}

resource "aws_iam_instance_profile" "ec2_ssm_profile" {
  name = "EC2SSMProfile"
  role = aws_iam_role.ec2_ssm_role.name
}

# -----------------------------------------------------------
# CloudWatch Rule to Stop EC2 After 3 Hours
# -----------------------------------------------------------
resource "aws_cloudwatch_event_rule" "shutdown_rule" {
  name                = "ShutdownEC2After3Hours"
  description         = "Stops the EC2 instance after 3 hours"
  schedule_expression = "rate(3 hours)"
}

data "aws_caller_identity" "current" {}

resource "aws_cloudwatch_event_target" "shutdown_target" {
  rule      = aws_cloudwatch_event_rule.shutdown_rule.name
  target_id = "StopEC2"
  arn       = "arn:aws:lambda:${var.region}:${data.aws_caller_identity.current.account_id}:function:StopEC2Function"

  input = jsonencode({
    InstanceId = aws_instance.rust_server.id
  })
}

resource "aws_iam_policy" "cloudwatch_ssm_policy" {
  name        = "CloudWatchSSMInvokePolicy"
  description = "Allows CloudWatch to invoke SSM documents"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = "ssm:SendCommand"
      Resource = "*"
    }]
  })
}

resource "aws_iam_role" "cloudwatch_role" {
  name = "CloudWatchSSMInvokeRole"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "events.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy_attachment" "cloudwatch_ssm_attachment" {
  role       = aws_iam_role.cloudwatch_role.name
  policy_arn = aws_iam_policy.cloudwatch_ssm_policy.arn
}
