

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
export DATABASE_URL=tracer-cluster-production.cluster-ro-cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432/tracer_db
```

## Running A Local Environment Test
init --pipeline-name rnaseq-demo-123 --environment demo --user-operator vincent --pipeline-type rnaseq
