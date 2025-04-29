
<h1 align="left">
🦡 Tracer Linux Agent
</h1>

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
