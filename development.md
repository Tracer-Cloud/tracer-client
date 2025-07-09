# Developer Notes for Getting Started with Tracer

This page provides additional notes for developers installing and testing Tracer locally.

<br />

## Installing Tracer With One Line of Code

Install Tracer from the `main` branch:

```bash
curl -sSL https://install.tracer.cloud/ | bash && source ~/.bashrc && source ~/.zshrc
```

Install Tracer from a custom branch, e.g. `custom-client` (Requires branch to be a pull request):

```bash
curl -sSL https://install.tracer.cloud/ | CLI_BRANCH="custom-client" bash && source ~/.bashrc
```

Use Installer from a custom branch, e.g. `custom-installer` (Requires branch to be a pull request):

```bash
curl -sSL https://install.tracer.cloud/ | INS_BRANCH="custom-installer" bash && source ~/.bashrc
```

Click the 'Open In Github Codespaces' button to use GitHub Codespaces.

Once in Codespaces, the environment comes with:
Tracer pre-installed and Docker running a minimal Nextflow example. Here, you need to run the tracer init command showcased in the next step.
