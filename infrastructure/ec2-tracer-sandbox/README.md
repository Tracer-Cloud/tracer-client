# Gameplan for fast startup
Infrastructure works:
- docker-compose build integrations_tests parallel_tests

## Notes So Far
- We use Graviton3 for instance type because they have superfast startup times.
- When enabling IAM rules it takes much much longer. We can consider doing only web based instance connect. 


## Roadmap:
- add a git ignore file for the terraform state.
- Single EC2 instance takes 14 seconds to provision with ubuntu 22.04 on a t3 micro, the same on a c7. 
- It has been possible to spin up a whole instance in 14 seconds. 
- Modifying can take 70 seconds (1m10s)
- Destroying the instance takes 40 seconds.



# Terraform Configuration Plan for AWS Cloud9 Environment
This Terraform configuration is designed to quickly provision a complete development environment on AWS Cloud9. It includes an EC2 instance with a custom image that comes pre-installed with essential tools and libraries, along with additional storage, automated GitHub integration, and a quick-access URL.

Features
AWS Cloud9 Environment: Provision an AWS Cloud9 environment backed by an EC2 instance.
Extra Storage: Attach an additional 100‑GB EBS volume to the instance for increased storage capacity.
Custom Instance Image: Pre-install the following tools and packages on the Cloud9 instance:
AWS CLI v2: For managing AWS services from the command line.
Docker: Installed and configured to automatically start its service.
Git: With an automated clone of your GitHub repository.
Rust: Installed via rustup to manage Rust toolchains.
Nextest: Installed via Cargo to run Rust tests efficiently.
System Packages: Install essential packages such as lld, clang, and necessary runtime libraries.
GitHub Integration: Automatically clone the Tracer-Cloud/tracer-client repository into the Cloud9 environment.
Output Cloud9 URL: After provisioning, output the Cloud9 IDE URL for immediate access from your terminal.
Prerequisites
Ensure the following tools are installed and configured on your local machine before running the Terraform configuration:

AWS CLI
Docker
Git
Detailed Steps
Provision the AWS Cloud9 Environment

Use Terraform's AWS provider to create a Cloud9 environment.
This will provision an EC2 instance configured to run the Cloud9 IDE.
Attach an Extra EBS Volume

Attach an additional 100‑GB EBS volume to the EC2 instance.
Configure the volume for additional storage required by your projects or tools.
Bootstrapping with a Custom Image

Utilize a custom AMI or user data script to install the required software:
AWS CLI v2: For interacting with AWS.
Docker: With its service enabled and running.
Git: To automatically clone your GitHub repository.
Rust and Nextest: Rust via rustup and Nextest via Cargo.
Essential System Packages: Such as lld, clang, and runtime libraries.
This ensures that every new instance is ready for development without manual intervention.
Clone the GitHub Repository

Integrate a step (via user data or a configuration management tool) to clone the repository from Tracer-Cloud/tracer-client automatically into the Cloud9 environment.
Output the Cloud9 URL

Configure Terraform to output the Cloud9 environment URL once the provisioning is complete.
This enables quick access to your IDE directly from the terminal.
Best Practices & Additional Considerations
Security and Access:

Ensure that the proper IAM roles and policies are set up for secure access to AWS resources.
Use Terraform variables to manage sensitive data, and consider integrating with AWS Secrets Manager.
Modular Design:

Organize your Terraform configuration into reusable modules. For example, separate modules for EC2 provisioning, EBS volume attachment, and Cloud9 setup can simplify maintenance and reusability.
Error Handling & Logging:

Implement robust error handling and logging within your user data scripts or configuration management tools to help debug issues during provisioning.
Environment Variables:

Use Terraform variables and environment-specific configuration files to manage differences between development, staging, and production environments.
How to Use
Clone the Repository:
Clone the repository containing your Terraform configuration.

Initialize Terraform:

bash
Copy
terraform init
Review the Plan:

bash
Copy
terraform plan
Apply the Configuration:

bash
Copy
terraform apply
Access Your Environment:
Once the configuration is applied, use the output Cloud9 URL to access your development environment.