<h2 align="left">
🦡 Tracer Documentation: Cloud Deployment, Linux Agent and Getting Your First Wokrflow Up and Running
</h2>

# Tracer Tutorial: Monitoring Your First Nextflow Pipeline on AWS
## Overview
This tutorial guides you through setting up Tracer monitoring for a Nextflow pipeline on AWS. You'll learn how to deploy the monitoring infrastructure, launch and verify a pipeline, and visualize its execution in Grafana.

## Example Use Case
- Real-time monitoring of Nextflow genomics pipelines on AWS

## Prerequisites: 
- Have setup [Tracer monitoring stack on AWS](https://www.notion.so/tracercloud/One-Command-Terraform-Deployment-on-AWS-1bb23c8b6bf5807c871ec64a1ec5aeae?pvs=4)
- Verify you have tracer-launch-template[UNIQUE ID] in your AWS console under EC2/LaunchTemplates 
- Verify you have setup Amazon Managed Grafana with a Tracer workspace

## Walkthrough Part 1. Setup 
- Step 1. Launch a new EC2 instance from the template launchers (Exmplae: tracer-launch-template20250321113516913800000003, Launch Template ID: lt-0fe7119f40a472f97)
- Step 2. Connect with AWS Instance Connect (you might have to wait 1 minute for the instance to provision)
- Step 3. Verify tracer is running and installed correctly by running $tracer info, the default pipeline name is launch-template but we will now proceed and give it a unique name
- Step 4. Stop the current pipline run and give it a unique new pipeline name such as $tracer stop && $tracer init --pipeline-name rnaseq-demo-123 --tags demo ec2
- Step 5. Run $tracer info in your terminal after connecting. And Copy and paste the Grafana Url in a different browser tab
- Step 6. Go to Grafana's Dashboard "All pipeline view" 
- Step 7. Verify that the pipeline is running correctly. You should see your unique name of the pipeline. You are now setup correctly. 

## Walkthrough Part 2. Running A Nextflow Pipeline 
- Step 9. Go to the directory with Tracer's example workloads. 
- Step 10. Run An RNA-seq Pipeline Locally $make test_rnaseq
- Step 11. Navigate to Grafana, to and Validate that the Nextflow pipeline is running by going 

## Walkthrough Part 3. Testing Plan For March 21st 
### Objective 1. Ease of use of the tracer binary. 
Questions: 
- 
- 
- 

### Objective 2. Ease of understanding of the Grafana Dashboards 
Questions: 
- What do you want to see. 
- Users can categorize pipelines with tags (e.g., "david, local, env"), improving organization and searchability.
- At which process step in the pipeline am I currently at? 

 Installation of the platform. 
- 

### Objective 3. Deployment of tracer infrastructure 
Questions:
- 
- 
- 


## Table of Contents
- 📚 [Documentation](./DOCUMENTATION.md) – Learn more about Tracer’s features and capabilities
- 🤝 [Contributing](./CONTRIBUTING.md) – Join the community and contribute to Tracer
- 🔍 [Examples](./EXAMPLES.md) – Explore real-world use cases and sample implementations
