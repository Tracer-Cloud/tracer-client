#!/bin/bash

set -e  # Exit on error

# Check if DATABASE_URL is passed as an argument
if [[ -n "$1" ]]; then
    DATABASE_URL="$1"
else
    # Fall back to environment variables if no argument is given
    if [[ -z "$DATABASE_URL" ]]; then
        : "${DB_USER:?DB_USER is not set}"
        : "${DB_PASS:?DB_PASS is not set}"
        : "${DB_HOST:?DB_HOST is not set}"
        : "${DB_PORT:=5432}"
        : "${DB_NAME:=tracer_db}"

        # Encode the password
        ENCODED_PASS=$(python3 -c "import urllib.parse; print(urllib.parse.quote('$DB_PASS'))")

        # Construct the database URL
        DATABASE_URL="postgres://${DB_USER}:${ENCODED_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
    fi
fi

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



