<h2 align="left">
Tracer Linux Agent: Observability for Scientific HPC Workloads
</h2>

![Tracer Banner](docs/images/tracer-banner-image.jpeg)

## What Is Tracer and Why Use It? 
- Tracer is a monitoring solution that optimizes HPC workloads for speed and cost efficiency. It features a one-line install Linux agent and instant dashboards for real-time insights into scientific computing environments.

- Unlike industry agnostic monitoring agents, Tracer structures DevOps data for scientific pipelines, providing clear visibility into pipeline stages and execution runs. In environments like AWS Batch, users struggle to track which processes or containers belong to the same pipeline and frequently lose logs from failed containers, making debugging difficult.

- Tracer solves this by intelligently organizing and labeling pipelines, execution runs, and steps. Because it runs directly on Linux, it requires no code changes and supports any programming language, unlike point solutions that work only with one framework. This makes integration effortless even across multi-workload IT environments, including AlphaFold, Slurm, Airflow, Nextflow and also local Bash scripts.

- Architected for regulated industries, it ensures enterprise-grade security, with data never leaving your infrastructure, which is not the case with solutions such as DataDog. 

## Key Features 
New metrics that help you speed up your pipelines and maximize your budget:
- Time and cost per dataset processed
- Execution duration and bottleneck identifcation for each pipeline step
- Cost attribution across pipelines, teams, and environments (dev, CI/CD, prod)

<br />

<img src="./docs/images/GitHubDemo1.gif" alt="Demo of tracer" width="80%"> 

## ⚡️ More Powerful Capabilities:
- Unified Monitoring: Track all your HPC pipelines in a single, centralized dashboard
- Faster Debugging: Identify CPU, RAM, and I/O bottlenecks instantly, and never lose AWS Batch container logs again
- Optimization & Savings: Spot computional waste and cut costs by up to 45% in computational biology workloads
- Enterprise-Grade Security: Airgapped deployment keeps all data within your infrastructure
- Upcoming Roadmap:
    - Q1 2025: Advanced Cost Attribution
    - Q2 2025: Performance Optimization for HPC
    - H2 2025: Automated Error Database

## Quickstart Installation On EC2
Prerequisites: 
- Linux EC2 (Ubuntu or Amazon Linux),  Intel x86, c6i.2xlarge, 8 vCPUs 8, 32+ GiB RAM
- AWS credentials exported to environment with access to RDS Postgres Database
- Amazon Managed Grafana pointed to Database
- Link to setting up [Nextflow sandbox](https://github.com/Tracer-Cloud/tracer-test-pipelines-bioinformatics/tree/main/frameworks/nextflow)

### 1. Install Tracer With One Line of Code
Run the following command to install Tracer on your Linux Ubuntu system (the following installation command points to the latest development binary build from the `main` branch): 
```bash
curl -sSL https://install.tracer.cloud/installation-script-development.sh | bash && source ~/.bashrc
```
### 2. Initialize a Pipeline
Set up your pipeline by specifying a name:
```bash
tracer init --pipeline-name airflow_test --environment sandbox_test --user-operator vincent --pipeline-type rnaseq
 ```

### 3. Infrastructure Setup  
You can run a single command from this repository to provision the required infrastructure:
- AWS account with access to a PostgreSQL database
- Grafana instance (Amazon Managed Grafana recommended)

```bash
cd infrastructure && terraform apply
 ```

### 4. View Instant Dashboards 
Run the following command to retrieve your dashboard URL:

```bash
tracer info
 ```

Open the link to access real-time dashboard insights into your computational workloads:

<img width="711" alt="image" src="https://github.com/user-attachments/assets/585d3061-c895-47d9-a091-bb1598d95cae" />



# Development Troubleshooting 

## How To Test Locally (Cloud Services)

```
make test-tracer
```


## How To Test AWS Batch 
- Spin up Sandbox with EC2 launch template
- Go to folder test-bioinformatics-packages  and run the test file 


```bash
sudo su - ubuntu && cd test-bioinformatics-packages 
```
```bash
tracer init --pipeline-name aws_batch_test --environment sandbox --user-operator vincent --pipeline-type aws_batch_rnaseq

```
```bash
make test_rnaseq_aws_batch
```

- make test_rnaseq_aws_batch

## Running Cargo Test (Not Working Per April 15th 2025)
If you get an error during testing 

```bash

---- alert stdout ----
thread 'alert' panicked at /Users/janvincentfranciszek/.cargo/registry/src/index.crates.io-6f17d22bba15001f/sqlx-postgres-0.8.3/src/testing/mod.rs:78:44:
DATABASE_URL must be set: EnvVar(NotPresent)
```

Then do:

```bash

export DATABASE_URL=tracer-cluster-production.cluster-ro-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432/
```

## eBPF

See [./src/ebpf/README.md](./src/ebpf/README.md) for guidance when extending the eBPF module.

## Running A Local Environment Test
```bash

init --pipeline-name rnaseq-demo-123 --environment demo --user-operator vincent --pipeline-type rnaseq
```


## Table of Contents
- [🎯 Tracer Tutorial](docs/README.MD) - Tracer Tutorial: Monitoring Your First Nextflow Pipeline on AWS
- [🔍 Examples](docs/EXAMPLES.md) – Explore real-world use cases 
- [🛣️ Infrastructure Setup](docs/INFRASTRUCTURE_SETUP.md) – 1 Command deployment
- [📚 Development](DOCUMENTATION.md) – Learn more about how to setup your development environment
- [🤝 Contributing](docs/CONTRIBUTING.md) – Join the community and contribute


## Mission

> *"The goal of Tracer's Rust agent is to equip scientists and engineers with DevOps intelligence to efficiently harness massive computational power for humanity's most critical challenges."*
