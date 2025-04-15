

# Development Troubleshooting 

## How To Test 

```
make test-tracer
```

## Running Cargo Test (Not Working Per April 15th 2025)
If you get an error during testing 

```bash

---- alert stdout ----
thread 'alert' panicked at /Users/janvincentfranciszek/.cargo/registry/src/index.crates.io-6f17d22bba15001f/sqlx-postgres-0.8.3/src/testing/mod.rs:78:44:
DATABASE_URL must be set: EnvVar(NotPresent)
```

Then do:

```bash
export DATABASE_URL=tracer-cluster-v2-instance-1.cdgizpzxtdp6.us-east-1.rds.amazonaws.com:5432/tracer_db
```