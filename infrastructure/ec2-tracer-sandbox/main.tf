provider "aws" {
  region  = var.region
  profile = "default"
}

data "aws_vpc" "default" {
  default = true
}

# -----------------------------------------------------------
# Fetch the latest Ubuntu 22.04 LTS AMI
# -----------------------------------------------------------
data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"] # Canonical
  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-arm64-server-*"]
  }
}

# -------------------------------------
# Resouce for generating random string.
# -------------------------------------
resource "random_string" "suffix" {
  length  = 8
  special = false
  upper   = false
}


# ---------------------------------------------
# EC2 Instance Deployment
# ---------------------------------------------
module "ec2_common" {
  source      = "../modules/ec2_common"
  name_suffix = random_string.suffix.result
  vpc_id      = data.aws_vpc.default.id
}


resource "aws_instance" "rust_server" {
  ami                    = data.aws_ami.ubuntu.id
  instance_type          = var.instance_type
  key_name               = var.key_name
  iam_instance_profile   = module.ec2_common.iam_instance_profile_name
  vpc_security_group_ids = [module.ec2_common.security_group_id]

  metadata_options {
    http_tokens                 = "optional"
    http_put_response_hop_limit = 1
    http_endpoint               = "enabled"
  }

  root_block_device {
    volume_size = var.root_volume_size
    volume_type = var.root_volume_type
  }

  tags = {
    Name = "Rust-EC2-Instance-${random_string.suffix.result}"
  }

  user_data = templatefile("${path.module}/script-install-deps.sh", {
    role_arn = module.ec2_common.service_role_arn
    api_key  = var.api_key
  })
}
