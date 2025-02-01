#!/bin/bash

# Check for required environment variables
if [ -z "$BEARER" ]; then
    echo "Error: BEARER environment variable is required"
    exit 1
fi

# Set default BASE_PATH if not provided
BASE_PATH=${BASE_PATH:-"http://127.0.0.1:9950"}

echo "Fetching all inboxes..."
inboxes_response=$(curl "$BASE_PATH/v2/all_inboxes" \
     -H "Authorization: Bearer $BEARER" \
     -H 'Content-Type: application/json; charset=utf-8')

# Extract job IDs using jq
job_ids=$(echo $inboxes_response | jq -r '.[] | .inbox_id | split("::")[1]')

# Counter for removed jobs
removed_count=0

# Remove each job
for job_id in $job_ids; do
    echo "Removing job: $job_id"
    remove_response=$(curl "$BASE_PATH/v2/remove_job" \
         -X POST \
         -H "Authorization: Bearer $BEARER" \
         -H 'Content-Type: application/json' \
         -d $"{\"job_id\": \"$job_id\"}")
    
    # Check if removal was successful
    if echo "$remove_response" | jq -e '.status == "success"' > /dev/null; then
        ((removed_count++))
        echo "Successfully removed job: $job_id"
    else
        echo "Failed to remove job: $job_id"
        echo "Response: $remove_response"
    fi
done

echo "Finished removing jobs. Total removed: $removed_count" 