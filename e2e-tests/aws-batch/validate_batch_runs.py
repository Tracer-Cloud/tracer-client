import json
import sys
import boto3
import time
from pathlib import Path

def check_nextflow_completion(work_dir):
    """Check if Nextflow pipeline completed successfully"""
    try:
        with open(Path(work_dir) / ".nextflow.log") as f:
            return "Workflow completed successfully" in f.read()
    except:
        return False

def check_processing_times(s3_bucket="tracer-nxf-outputs"):
    """Verify we captured processing times for datasets"""
    s3 = boto3.client('s3')
    try:
        response = s3.list_objects_v2(
            Bucket=s3_bucket,
            Prefix="metrics/"
        )
        if 'Contents' not in response:
            return False
        
        # Check if we have timing data in any of the metrics files
        for obj in response['Contents']:
            data = s3.get_object(Bucket=s3_bucket, Key=obj['Key'])
            metrics = json.loads(data['Body'].read())
            if 'processing_time' in metrics:
                return True
        return False
    except:
        return False

def main():
    work_dir = sys.argv[1] if len(sys.argv) > 1 else "work"
    
    # Give S3 some time to sync
    time.sleep(30)
    
    pipeline_success = check_nextflow_completion(work_dir)
    times_captured = check_processing_times()
    
    print(f"Pipeline completion: {pipeline_success}")
    print(f"Processing times captured: {times_captured}")
    
    if not (pipeline_success and times_captured):
        sys.exit(1)

if __name__ == "__main__":
    main()