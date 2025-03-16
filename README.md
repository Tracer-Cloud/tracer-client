<h2 align="left">
🦡 Tracer Linux Agent – Observability for High-Performance Computing (HPC) Workloads
</h2>

> *"The goal of Tracer's Rust agent is to equip scientists and engineers with DevOps intelligence to efficiently harness massive computational power for humanity's most critical challenges."*

![Tracer Banner](docs/images/tracer-banner-image.jpeg)

## ⚡️ Features 
- Unified Monitoring: Centralized status monitoring of your HPC pipelines in a single dashboard
- Cost Attribution: Allocate cloud costs across pipelines, teams, and environments (dev, CI/CD, prod)
- Optimization & Savings: Reduce compute waste and cut costs by up to 45% in computational biology workloads
- Faster Debugging: Detect CPU, RAM, and I/O bottlenecks instantly, never lose your container logs agains
- Airgapped Security: data remains inside your own enterprise systems
- Upcoming Roadmap:
    - Cost Attribution (Q1 2025)
    - Speed optimization for HPC (Q2 2025)
    - Error database (H2 2025)

<br />

![Tracer Dashboards](docs/images/20250316-kpi-dashboard.png)

## 🚀 Quickstart Installation
### 1. Infrastructure Setup  
Get started in minutes. Ensure you have:
- AWS account with access to a PostgreSQL database
- Grafana instance (Amazon Managed Grafana recommended)

You can run a single command from this repository to provision the required infrastructure:

```bash
cd infrastructure && terraform apply
 ```

### 2.Install Tracer With One Line of Code
Run the following command to install Tracer on your Linux Ubuntu system:
```bash
curl -sSL https://install.tracer.cloud/installation-script.sh | bash && source ~/.bashrc
 ```
### 3.Initialize a Pipeline
Set up your pipeline by specifying a name:
```bash
tracer init --pipeline-name <YOUR_PIPELINE_NAME>
 ```
### 4. View Instant Dashboards 
Run the following command to retrieve your dashboard URL and open the link to access real-time insights into your computational workloads

## Table of Contents
- [🛣️ Infrastructure Setup](docs/INFRASTRUCTURE_SETUP.md) – 1 Command deployment
- [📚 Documentation](DOCUMENTATION.md) – Learn more about Tracer’s capabilities
- [🤝 Contributing](docs/CONTRIBUTING.md) – Join the community and contribute
- [🔍 Examples](docs/EXAMPLES.md) – Explore real-world use cases 

