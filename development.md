# Developer Notes for Getting Started with Tracer

This page provides additional notes for developers installing and testing Tracer locally.

<br />

## Installing Tracer With One Line of Code

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
