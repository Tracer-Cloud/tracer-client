terraform {
  backend "s3" {
    bucket         = "tracer-cloud-terraform-state"
    key            = "launch_template/terraform.tfstate"
    region         = "us-east-1"
    encrypt        = true
    dynamodb_table = "tf-launch-template-state"
  }
}

data "aws_vpc" "default" {
  default = true
}

provider "aws" {
  region  = var.region
  profile = "default"
}

module "ec2_common" {
  source      = "../modules/ec2_common"
  name_suffix = "launch-template"
  vpc_id      = data.aws_vpc.default.id
}

# FIXME: Recreate AMIs to use main branch instead performing checkout in deployment script

# ---------------------------
# EC2 Launch Template
# ---------------------------
resource "aws_launch_template" "tracer_launch_template" {
  name_prefix   = "tracer-launch-template"
  image_id      = var.ami_id
  instance_type = var.instance_type

  key_name = var.perm_key

  iam_instance_profile {
    name = module.ec2_common.iam_instance_profile_name
  }

  network_interfaces {
    associate_public_ip_address = true
    security_groups             = [module.ec2_common.security_group_id]
  }

  user_data = base64encode(templatefile("${path.module}/setup-tracer.sh", {
    role_arn = module.ec2_common.service_role_arn
    api_key  = var.api_key
  }))

}
