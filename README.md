<h2 align="left">
🦡 Tracer Linux Agent – Observability for High-Performance Computing (HPC) Workloads
</h2>

> *"The goal of Tracer's Rust agent is to equip scientists and engineers with DevOps intelligence to efficiently harness massive computational power for humanity's most critical challenges."*

![Tracer Banner](docs/images/tracer-banner-image.jpeg)

## ⚡️ Features 
Gain unprecedented insights into your HPC pipelines with KPIs like time and cost per dataset processed and execution cost per pipeline run, helping you to speed up your piplelines and make your budgets go farther

![Tracer Dashboards](docs/images/20250316-kpi-dashboard.png)

More Powerful Capabilities:
- Unified Monitoring: Track all your HPC pipelines in a single, centralized dashboard
- Cost Attribution: Track cloud costs across pipelines, teams, and environments (dev, CI/CD, prod)
- Faster Debugging: Identify CPU, RAM, and I/O bottlenecks instantly, and never lose AWS Batch container logs again
- Optimization & Savings: Spot computional waste and cut costs by up to 45% in computational biology workloads
- Enterprise-Grade Security: Airgapped deployment keeps all data within your infrastructure
- Upcoming Roadmap:
    - Q1 2025: Advanced Cost Attribution
    - Q2 2025: Performance Optimization for HPC
    - H2 2025: Automated Error Database

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

