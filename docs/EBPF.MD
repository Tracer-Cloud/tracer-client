# Documentation To Run eBPF Specific Functionality 
How to run eBPF:
- Linux Ubuntu v22
- Run the eBPF binary on Root(!) 
- sudo su - root

```bash 
# Installation May 1st 
sudo curl -sSL https://blaginin-eng-26-fix-ebpf-eve.tracer-client.pages.dev/installation-script-development.sh | bash && source ~/.bashrc 
```

```bash
# Run the pipeline as sudo 
sudo tracer init --pipeline-name ebpf_test --environment sandbox_test --user-operator vincent --pipeline-type rnaseq
```






## Running the eBPF build

```bash
RUST_LOG=info cargo run --config 'target."cfg(all())".runner="sudo -E"' -- init --pipeline-name testing --run-id parallel-tag --environment integrations --user-operator inte --pipeline-type generic --no-daemonize
```


# Tracer test 
cargo run init --pipeline-name airflow_test --environment sandbox_test --user-operator vincent --pipeline-type rnaseq
