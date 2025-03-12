#!/bin/bash

set -e  # Exit on error

: "${DB_USER:?DB_USER is not set}"
: "${DB_PASS:?DB_PASS is not set}"
: "${DB_HOST:?DB_HOST is not set}"
: "${DB_PORT:=5432}"
: "${DB_NAME:=tracer_db}"


# Encode the password
ENCODED_PASS=$(python3 -c "import urllib.parse; print(urllib.parse.quote('$DB_PASS'))")

# Construct the connection URL
DATABASE_URL="postgres://${DB_USER}:${ENCODED_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

echo "Using database URL: $DATABASE_URL"

# Check if sqlx is installed, install if missing
if ! command -v sqlx &> /dev/null; then
    echo "sqlx not found. Installing..."
    cargo install sqlx-cli --no-default-features --features postgres
fi

# Run migrations
echo "Running migrations..."
sqlx migrate run --database-url $DATABASE_URL

echo "✅ Migration completed successfully!"



