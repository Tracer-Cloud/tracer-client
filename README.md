<h1 align="left">
🦡 Tracer Linux Agent
</h1>

> *"The goal of Tracer's Rust agent is to equip scientists and engineers with DevOps intelligence to efficiently harness massive computational power for humanity's most critical challenges."*

![Tracer Banner](docs/images/tracer-banner-image.jpeg)

## 🚀 Quickstart Installation
Get started with Tracer in just a few steps.

### 1️⃣ Install Tracer  
Run the following command to install Tracer on your Linux system:
```bash
curl -sSL https://install.tracer.cloud/installation-script.sh | bash && source ~/.bashrc
 ```
### 2️⃣ Initialize a Pipeline
Set up your pipeline by specifying a name:
```bash
tracer init --pipeline-name <YOUR_PIPELINE_NAME>
 ```

## How to Test Tracer:
- Ensure you have docker running
- Use cargo nextest run to run the tests
   ```rust
   cargo nextest run
   ```


## How to check if Tracer Daemon Is Running:

```bash
$ ps -e | grep tracer
```

---

## Running S3 Integration

This section outlines the requirements and setup necessary to use the S3 integration effectively. The S3 client supports flexible credential loading mechanisms.

### **Requirements**

1. **AWS Credentials**
   - Ensure your AWS credentials are available in one of the following locations:
     - `~/.aws/credentials` file with the appropriate profiles.
     - Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`).

2. **IAM Role (Optional)**
   - If running within an AWS environment (e.g., EC2, Lambda), you can use an IAM role to assume credentials automatically.

### **Initialization**

The S3 client initializes with the following options:
- **`profile`**: Load credentials from a named profile in the `~/.aws/credentials` file.
- **`role_arn`**: Assume an IAM role to obtain temporary credentials.
- **Fallback**: Automatically loads credentials from env when neither `profile` nor `role_arn` is provided.

#### Credential Sources

1. **Profile Name (`profile`)**:
   - Set up a profile in your `~/.aws/credentials` file.
   - Example:
     ```ini
     [my-profile]
     aws_access_key_id = YOUR_ACCESS_KEY_ID
     aws_secret_access_key = YOUR_SECRET_ACCESS_KEY
     ```
   - Pass the profile name as an argument to `new`.

2. **Assume Role (`role_arn`)**:
   - Provide a valid `role_arn` to assume an IAM role and retrieve temporary credentials. E.g: `"arn:aws:iam::123456789012:role/MyRole"`

3. **Default Credentials**:
   - If no `profile` or `role_arn` is provided, credentials are loaded automatically based on the default AWS configuration.

### **Docker Integration**

1. **Why use Docker with LocalStack**
   - To test the S3 integration, the client uses LocalStack, which is set up using Docker.
   - The Docker Compose file is located in the root of the repo.

2. **How to run Docker with LocalStack**
   - Ensure LocalStack is installed and running. You can start it using Docker:
     ```bash
     docker run -d -p 4566:4566 -p 4571:4571 localstack/localstack
     ```

---

### Grafana Loki 
- You need to start Grafana Loki sperately: docker-compose up -d loki
- Check if it is running: docker ps | grep loki  

### **Notes**

1. **Credential Resolution**
   - The function will panic if both `profile` and `role_arn` are provided.
   - It will also panic if no valid credentials are found during initialization.

2. **AWS Region**
   - Ensure the specified `region` matches the location of your S3 buckets.

---

## Development

### **Troubleshooting**

- If you encounter the error: `failed to run custom build command for 'openssl-sys v0.9.103'` or SSL-related issues, install the required dependencies:
  ```bash
  sudo apt install libssl-dev pkg-config
  ```

# Docker Container Registry

To speed up our CI pipeline, we utilize a custom Docker container registry on GitHub, known as the GitHub Container Registry (GCHR). This allows us to efficiently manage and deploy our Docker images.

### Steps to Use the Docker Container Registry
1. **Build the docker file**
   ```bash
   docker build -t rust-ci-arm64 -f Dockerfile .
   ```

2. **Tag the Docker Image**  
   Tag your Docker image with the appropriate repository name:
   ```bash
   docker tag rust-ci-arm64 ghcr.io/tracer-cloud/tracer-cloud:rust-ci-arm64
   ```
3. **Authenticate with the GitHub Container Registry**  
   Use your GitHub token to log in to the registry. This step is necessary for pushing images:
   ```bash
   echo $GITHUB_TOKEN | docker login ghcr.io -u Tracer-Cloud --password-stdin
   ```

4. **Push the Docker Image to the Registry**  
   Push the tagged image to the GitHub Container Registry:
   ```bash
   docker push ghcr.io/tracer-cloud/tracer-cloud:rust-ci-arm64
   ```


5. **Repeat Tagging and Pushing**  
   If you need to tag and push the image again, you can repeat the tagging and pushing steps:
   ```bash
   docker tag rust-ci-arm64 ghcr.io/tracer-cloud/tracer-cloud:rust-ci-arm64
   docker push ghcr.io/tracer-cloud/tracer-cloud:rust-ci-arm64
   ```

### Note
Ensure that your GitHub token has the necessary permissions to access the GitHub Container Registry.



## **Running Tracer Locally (Ideally a Linux Machine)**  

### **1. Create the Configuration Directory**  
Tracer requires a configuration directory. Create it with:  

```bash
mkdir -p ~/.config/tracer/
```

### **2. Create the Configuration File (`tracer.toml`)**  
This file will hold the necessary settings, such as AWS Initalization type(`Role ARN` or `Profile`), API key, and any other runtime configurations.  

```bash
touch ~/.config/tracer/tracer.toml
```

### **3. Setup Tracer with an API Key**  
Before running the tracer, you need to initialize it with an API key. Run:  

```bash
cargo run setup --api-key "your-api-key"
```

This step ensures that Tracer has the necessary authentication to send logs or traces to the backend.

### **4. Apply Bashrc Configuration (if needed)** 

This step sets up a custom Bash configuration to intercept and log relevant commands. It creates a .bashrc file inside the tracer config directory, defining aliases for monitored commands. This ensures that when a command runs, tracer logs its execution without interfering with normal operation.

Additionally, it redirects stdout and stderr to `/tmp/tracerd-stdout` and `/tmp/tracerd-stderr`, allowing users to track command outputs and errors. The setup persists across sessions by sourcing the custom `.bashrc` file in the user's shell configuration.

```bash
cargo run apply-bashrc
```

---

## **5. Configure AWS Credentials**  
If you're running Tracer on an **EC2 instance** or a local machine that interacts with AWS, ensure your AWS credentials are set up correctly.

- **Updating `tracer.toml` for AWS IAM Roles (EC2):**  
  Instead of using an `aws_profile`, modify `tracer.toml` to specify the AWS IAM Role ARN you want to assume:
  ```toml
  aws_role_arn = "arn:aws:iam::123456789012:role/YourRoleName"
  ```

---

## **6. Running Tracer as a Daemon**  
Tracer runs in the background as a daemon using the `daemonize` crate in Rust. This ensures it continues running after logout or system reboots.

- **Monitor Daemon Logs for Errors**  
  Since the tracer runs as a daemon, you won't see its output in the terminal. Check logs for debugging:  
  ```bash
  tail -f /tmp/tracerd.err
  ```
  This file contains runtime errors if something goes wrong.

---

## **Understanding `daemonize` in Rust**  
The [`daemonize`](https://docs.rs/daemonize/latest/daemonize/) crate helps create system daemons in Rust by handling:  
- **Forking the process** (so it runs in the background)  
- **Detaching from the terminal** (so it doesn't stop when you close the session)  
- **Redirecting logs to files** (important for debugging)  
- **Setting permissions and working directories**  

A simple Rust program using `daemonize` might look like this:  

```rust
use daemonize::Daemonize;
use std::fs::File;

fn main() {
    let log_file = File::create("/tmp/tracerd.log").unwrap();
    let error_file = File::create("/tmp/tracerd.err").unwrap();

    let daemon = Daemonize::new()
        .pid_file("/tmp/tracerd.pid") // Store PID
        .chown_pid_file(true)
        .working_directory("/") // Set working dir
        .stdout(log_file) // Redirect stdout
        .stderr(error_file) // Redirect stderr
        .privileged_action(|| println!("Tracer started!"));

    match daemon.start() {
        Ok(_) => println!("Daemon started successfully."),
        Err(e) => eprintln!("Error starting daemon: {}", e),
    }
}
```

This ensures Tracer runs continuously in the background.

---

## **And Voila! 🎉**
Your Tracer agent should now be running as a daemon on your Linux machine. If you encounter issues, check logs in `/tmp/tracerd.err`. 🚀



## Managing the Database with sqlx

To manage our PostgreSQL database schema and apply migrations, we use sqlx.

1. Install sqlx CLI

If you haven’t already, install sqlx with:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

2. Creating a New Migration

To create a new migration, run:
```bash
sqlx migrate add <migration_name>
```

This will generate two SQL files in the migrations/ directory:
	•	{timestamp}_<migration_name>.up.sql → Contains the SQL commands to apply the migration.
	•	{timestamp}_<migration_name>.down.sql → Contains the SQL commands to roll back the migration.

3. Running Migrations

To apply all pending migrations to your database:

```bash
sqlx migrate run
```

To revert the last applied migration:

sqlx migrate revert

### Important Note

Note that the compiler won’t pick up new migrations if no Rust source files have changed.
You can create a Cargo build script to work around this with:

This ensures that migrations are always detected and applied when building the project.

Here’s an improved version of the note with the clarification about embedding migrations:

4. Embedding Migrations in Your Application

Did you know you can embed your migrations in your application binary?
This allows your application to apply migrations automatically on startup.

After creating your database connection or pool, add:
```rust
sqlx::migrate!().run(<&your_pool OR &mut your_connection>).await?;
```
#### Important Note:
When embedding migrations, the compiler won’t detect new migrations unless a Rust source file has changed.
To ensure new migrations are always included, use a Cargo build script.
