
<h1 align="left">
🦡 Tracer Linux Agent
</h1>

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
  tail -f /tmp/tracer/tracerd.err
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
    let log_file = File::create("/tmp/tracer/tracerd.log").unwrap();
    let error_file = File::create("/tmp/tracer/tracerd.err").unwrap();

    let daemon = Daemonize::new()
        .pid_file("/tmp/tracer/tracerd.pid") // Store PID
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


### **Managing the Database with SQLX**

###  📌 Overview

We use `sqlx` to manage database schema migrations for our PostgreSQL database.  
Unlike typical setups where migrations run automatically in the Rust binary, **our approach separates migrations from the application** to ensure stability, consistency, and prevent breaking changes across multiple developers.

### **🚨 Why We Use Manual Migrations**
- We **use a single shared database** for development and testing.
- Running migrations in the Rust binary can **cause issues** if multiple developers apply conflicting migrations at different times.
- A **manual migration process** ensures everyone is on the same schema version before running the application.
- This also prevents errors due to version mismatches in `sqlx`, which can block developers from working.

---

### **🛠️ Installing SQLX CLI**
If you haven’t already installed `sqlx`, do so with:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

---

### **📝 Creating a New Migration**
To create a new migration, run:

```bash
sqlx migrate add <migration_name>
```

This will generate two SQL files in the `migrations/` directory:

- `{timestamp}_<migration_name>.up.sql` → Contains the SQL commands to apply the migration.
- `{timestamp}_<migration_name>.down.sql` → Contains the SQL commands to roll back the migration.

---

## **🚀 Database Migrations**  

### **🔄 GitHub Actions: Automated Migrations & Rollbacks**  

We use **GitHub Actions** to handle database migrations automatically when schema changes are merged into `main`.  

#### **How It Works:**  
✅ **Triggers migration** when PRs with schema changes (inside `./migrations`) are merged.  
✅ **Fetches DB password securely** from AWS Secrets Manager.  
✅ **Runs `migrate.sh`** to apply new migrations to the database.  
✅ **Rollback (`rollback.sh`) can be triggered manually** via GitHub Actions if needed.  

#### **Running Rollback in GitHub Actions**  
If an issue occurs with a migration, rollback can be executed manually:  
1. Navigate to **GitHub Actions**  
2. Select the **DB Migration Workflow**  
3. Run the **rollback job**, which executes `./rollback.sh`.  

---

### **🛠️ Running Migrations (Manual Approach)**  

Since we **do not run migrations automatically in the Rust binary**, we handle them using **separate migration scripts**.

#### **1️⃣ Setting the Database URL**  
The migration scripts accept the **database URL** in two ways:  
- **As an argument:**  
  ```bash
  ./migrate.sh "postgres://user:password@host:port/dbname"
  ```  
- **From environment variables:**

   - Exporting the db url
      ```bash
      export DATABASE_URL="postgres://user:password@host:port/dbname"
      ./migrate.sh
      ``` 
   - Exporting as parts
      ```bash
      export DB_USER="db_user"
      export DB_PASS="password"
      export DB_HOST="dbhost.com"
      export DB_PORT="5432"
      export DB_NAME="db"
      ./migrate.sh
      ```
**⚠️ Important:** If passing the **database URL as an argument**, ensure the **password is properly URL-encoded** to avoid issues with special characters (`@`, `:`, `#`, etc.). You can encode it using:  
```bash
python3 -c "import urllib.parse; print(urllib.parse.quote('your_password_here'))"
```  
Then replace `password` in the database URL with the encoded output.

#### **2️⃣ Running the Migration Script**  
To apply all pending migrations, use:  
```bash
./migrate.sh
```
#### **3️⃣ Rolling Back Migrations**  
To undo the last applied migration:  
```bash
./rollback.sh
```  
This scripts will:  
✅ Check if `sqlx` is installed (if not, it installs it).
  
✅ Connect to the database and **migrate or revert the last migration**.    

---

### **💡 How the Migration Process Works in the Code**  

- This approach enforces **intentionality** around schema changes—developers must apply migrations only when necessary, reducing unintended updates.  
- Since migrations are **manual**, developers can first test them on a separate test database before applying them to the main server.  
- While version mismatch errors may still occur, this process helps **limit their impact** by ensuring that migrations are only run when the changes are confirmed to work.
---

### **🚀 Deployment Process**

#### **🔄 Automatic Approach (Recommended)**  
1. **Submit schema changes** separately in a PR.  
2. Once approved, **merge the PR into `main`**.  
3. The **GitHub Actions workflow** triggers **automatic database migrations** and completes the update.  
4. After migration is confirmed, **submit a second PR** with the **code changes** that depend on the new schema.  
5. Merge the code PR, ensuring tests pass against the updated schema.  
6. If issue arises, trigger the rollback actions
This avoids breaking tests by ensuring schema changes are applied **before** the dependent code is introduced.

---

#### **🛠️ Manual Approach**  
If needed, migrations can be applied manually:  

1. **Apply the migration manually** with:  
   ```bash
   ./migrate.sh
   ```  
   _(Or pass the DB URL as an argument if needed.)_  
2. **Deploy the Rust binary**, assuming the database is up-to-date.  
3. If an issue arises, **use `./rollback.sh`** to undo the last migration.