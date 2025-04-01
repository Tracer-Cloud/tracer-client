# Tracer Tutorial: Monitoring Your First Nextflow Pipeline on AWS
This tutorial guides you through setting up Tracer monitoring for a Nextflow pipeline on AWS. You'll learn how to deploy the monitoring infrastructure, launch and verify a pipeline, and visualize its execution in Grafana.

## Example Use Case
- Real-time monitoring of Nextflow genomics pipelines on AWS

## Prerequisites: 
- Have access to an AWS account or ask your administrator
- Have setup [Tracer monitoring stack on AWS](https://www.notion.so/tracercloud/One-Command-Terraform-Deployment-on-AWS-1bb23c8b6bf5807c871ec64a1ec5aeae?pvs=4)
- Verify you have tracer-launch-template[UNIQUE ID] in your AWS console under EC2/LaunchTemplates 
- Verify you have setup Amazon Managed Grafana with a [Tracer workspace]()

## Walkthrough Part 1. Setup 
- Step 1. Launch a new EC2 instance from the template launchers (Example: tracer-launch-template20250321113516913800000003, Launch Template ID: lt-0fe7119f40a472f97)
- Step 2. Connect with AWS Instance Connect (you might have to wait 1 minute for the instance to provision)
- Step 3. Verify tracer is installed correctly by running  `tracer info`.

- Step 4a. Change user if you're signed in as root 

```bash
sudo su - ubuntu
```
- Step 4b. Start a new pipline run and give it a unique new pipeline name such as
```bash
tracer init --pipeline-name rnaseq-demo-123 --tags vincent,demo
```
- Step 5. Run $tracer info in your terminal after connecting. And Copy and paste the Grafana Url in a different browser tab
```bash
tracer info
```
- Step 6. Go to Grafana's Dashboard "All pipeline view" 
- Step 7. Verify that the pipeline is running correctly. You should see your unique name of the pipeline. You are now setup correctly. 

## Walkthrough Part 2. Running A Nextflow Pipeline 
- Step 9. Go to the directory with Tracer's example workloads. 
- Step 10. Run An RNA-seq Pipeline Locally:

```bash
su - ubuntu
```
```bash
cd ~/tracer-test && cd tracer-test-bioinfomatics
```
```bash
make test_rnaseq
```

- Step 11. Navigate to Grafana, to and Validate that the Nextflow pipeline is running by going to the Grafana Dashboard: [https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com](https://g-3f84880db9.grafana-workspace.us-east-1.amazonaws.com)


## Walkthrough Part 3. What To Explore Afterwards
- View the first pipeline overview page. 
- Click the current pipeline that you have just created to see an overview of all the individual pipeline runs. 
- Click on the most recent run to see your pipeline progress. 


## Table of Contents
- 📚 [Documentation](./DOCUMENTATION.md) – Learn more about Tracer’s features and capabilities
- 🤝 [Contributing](./CONTRIBUTING.md) – Join the community and contribute to Tracer
- 🔍 [Examples](./EXAMPLES.md) – Explore real-world use cases and sample implementations
