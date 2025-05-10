#!/bin/bash

# Get script directory
SCRIPT_DIR="$(dirname "$(realpath "$0")")"

# Dataset directory
DATASET_DIR="$SCRIPT_DIR/../test-data-samples"

# Ensure the dataset directory exists
mkdir -p "$DATASET_DIR"

# Generate test .fa files
for i in {1..3}; do
    echo -e ">test_sequence\nAGCTTAGCTA" > "$DATASET_DIR/test$i.fa"
done

echo "Generated test datasets: test1.fa, test2.fa, test3.fa"

# Process each dataset using native tools (no Python)
for file in "$DATASET_DIR"/*.fa; do
    echo "Processing dataset: $file"
    # Use gzip to compress the file (gzip uses zlib, which is in our target list)
    gzip -c "$file" > "$file.gz"
    # Decompress the file
    gzip -d -c "$file.gz" > "$file.decompressed"
    # Wait to make sure the process is captured
    sleep 2
    echo "Finished processing: $file"
done

# Cleanup after processing
rm -rf "$DATASET_DIR"/
echo "Dataset processing completed."
