<h2 align="left">
Tracer Linux Agent: Observability for Scientific HPC Workloads
</h2>

![Tracer-Banner](https://github.com/user-attachments/assets/5bbbdcee-11ca-4f09-b042-a5259309b7e4)


## What Is Tracer and Why Use It? 
- Tracer is a system-level monitoring platform purpose-built for scientific computing. It is a a one-line install Linux agent and instant dashboards to give you insights into pipeline performance and cost optimization. 

- Unlike industry agnostic monitoring agents, Tracer structures DevOps data for scientific pipelines, providing clear visibility into pipeline stages and execution runs. In environments like AWS Batch, where processes and containers are loosely connected, users struggle to understand which processes belong to which pipeline run, and frequently lose logs from failed containers, making debugging difficult.

- Tracer solves this by intelligently organizing and labeling pipelines, execution runs, and steps. Because it runs directly on Linux, it requires no code changes and supports any programming language, unlike point solutions that work only with one framework. This makes integration effortless even across multi-workload IT environments, including AlphaFold, Slurm, Airflow, Nextflow and also local Bash scripts.

- Architected for regulated industries, it ensures enterprise-grade security, with data never leaving your infrastructure, which is not the case with solutions such as DataDog.

<br />

![image](https://github.com/user-attachments/assets/c59b2db5-81c0-4d92-b614-e8733a0303b9)

<br />

## Key Features

New metrics that help you speed up your pipelines and maximize your budget:

- Time and cost per dataset processed
- Execution duration and bottleneck identification for each pipeline step
- Cost attribution across pipelines, teams, and environments (dev, CI/CD, prod)
  Overall, making sense of scientific toolchains with poor/no observability.

<br />

## Get Started

### 1. Access the Sandbox

The easiest way to get started with Tracer is via our **browser-based sandbox**:  
👉 [https://sandbox.tracer.cloud/](https://sandbox.tracer.cloud/)

→ Click **“Get started”** to launch a guided onboarding experience tailored to your preferred tech stack — *no AWS credentials or setup required*.

### 2. Install Tracer With One Line of Code

Install Tracer with this single command:

```bash
curl -sSL https://install.tracer.cloud/ | sudo bash && source ~/.bashrc && source ~/.zshrc
```

```bash
curl -sSL https://install.tracer.cloud/installation-script-development.sh | sudo bash && source ~/.bashrc && source ~/.zshrc
```

To get the binary corresponding to the `main` branch you just have to put `-s main` after the bash command like in the following example

```bash
curl -sSL https://install.tracer.cloud/installation-script-development.sh | sudo bash -s main && source ~/.bashrc
```

To get your pr binary use `bash -s <branch-name>` like in the following example

```bash
curl -sSL https://install.tracer.cloud/installation-script-development.sh | sudo bash -s feature/my-branch && source ~/.bashrc
```

Click the 'Open In Github Codespaces' button to use GitHub Codespaces.

Once in Codespaces, the environment comes with:
Tracer pre-installed and Docker running a minimal Nextflow example. Here, you need to run the tracer init command showcased in the next step.

### 3. Initialize a Pipeline

Follow the sandbox instructions to launch your own pipeline or run a script to launch a **demo bioinformatics pipeline** (Nextflow, WDL, and more) from our [nextflow-test-pipelines-bioinformatics](https://github.com/Tracer-Cloud/nextflow-test-pipelines) repository.

### 4. Monitor Your Pipeline With Our Grafana Dashboard

Access the Tracer monitoring dashboard on **Grafana** to watch your pipeline in action, including:

- Real-time execution metrics  
- Pipeline stages  
- Resource usage across runs  

→ The sandbox will guide you through creating your personal account and navigating the Grafana interface.

<br />

## Mission

> _"The goal of Tracer's Rust agent is to equip scientists and engineers with DevOps intelligence to efficiently harness massive computational power for humanity's most critical challenges."_
