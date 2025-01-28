#!/bin/bash

# Performance Testing Script - create_jobs.sh
# ==========================================
#
# Description:
# ------------
# This script creates test jobs with simulated chat conversations for performance testing purposes.
# It creates a specified number of jobs through the Shinkai API, each containing a simulated
# conversation about React and WebSocket implementation.
#
# Use Cases:
# ----------
# - Load testing the job creation system
# - Performance testing with multiple concurrent jobs
# - Testing message handling capabilities
#
# Prerequisites:
# -------------
# - Bash shell
# - curl command-line tool
# - Access to a running Shinkai API instance
#
# Environment Variables:
# --------------------
# JOBS_COUNT    - Number of jobs to create (default: 1)
# AUTH_TOKEN    - Bearer token for API authentication
#                 (default: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ikpva")
# API_BASE_URL  - Base URL of the Shinkai API (default: "http://127.0.0.1:9950")
# DELAY_MS      - Delay between requests in milliseconds (default: 100ms)
#
# Usage Examples:
# --------------
# 1. Create a single job with default settings:
#    ./create_jobs.sh
#
# 2. Create multiple jobs:
#    JOBS_COUNT=5 ./create_jobs.sh
#
# 3. Use custom API endpoint:
#    API_BASE_URL="http://my-api-server:9950" ./create_jobs.sh
#
# 4. Combine multiple settings:
#    JOBS_COUNT=3 API_BASE_URL="http://custom-server:9950" AUTH_TOKEN="your-token" ./create_jobs.sh
#
# Example Output:
# -------------
# Creating 3 jobs...
# Processing job 1 of 3
# Created job with ID: abc123
# Added messages to job: abc123
# ----------------------------------------
# Processing job 2 of 3
# Created job with ID: def456
# Added messages to job: def456
# ----------------------------------------
# Processing job 3 of 3
# Created job with ID: ghi789
# Added messages to job: ghi789
# ----------------------------------------
# Completed creating 3 jobs

# Default values for environment variables
JOBS_COUNT=${JOBS_COUNT:-1}
AUTH_TOKEN=${AUTH_TOKEN:-"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ikpva"}
API_BASE_URL=${API_BASE_URL:-"http://127.0.0.1:9950"}
DELAY_MS=${DELAY_MS:-100}  # Default delay of 100ms between requests

# Function to create a job and add messages
create_job_and_add_messages() {
    # Create job
    response=$(curl -s -X "POST" "$API_BASE_URL/v2/create_job" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H 'Content-Type: application/json; charset=utf-8' \
        -d '{
            "llm_provider": "llama3_1_8b",
            "job_creation_info": {
                "scope": {
                    "network_folders": [],
                    "vector_fs_folders": [],
                    "vector_fs_items": [],
                    "local_vrpack": [],
                    "local_vrkai": []
                },
                "is_hidden": false
            }
        }')

    # Extract job_id from response
    job_id=$(echo $response | grep -o '"job_id":"[^"]*"' | cut -d'"' -f4)
    
    if [ -z "$job_id" ]; then
        echo "Failed to get job_id from response"
        return 1
    fi

    echo "Created job with ID: $job_id"

    # Sleep for the specified delay
    sleep $(echo "scale=3; $DELAY_MS/1000" | bc)

    # Add messages to the job
    curl -s -X "POST" "$API_BASE_URL/v2/add_messages_god_mode" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H 'Content-Type: application/json; charset=utf-8' \
        -d "{
            \"job_id\": \"$job_id\",
            \"messages\": [
                {
                    \"parent\": \"\",
                    \"content\": \"Message ${i}: I'm working on a React component that needs to handle real-time updates. What's the best approach?\",
                    \"job_id\": \"$job_id\",
                    \"files_inbox\": \"\"
                },
                {
                    \"parent\": \"\",
                    \"content\": \"For real-time updates in React, you have several options. The most common approaches are WebSockets or Server-Sent Events (SSE). WebSockets are great for bi-directional communication, while SSE is simpler if you only need server-to-client updates. Would you like me to explain the implementation details for either approach?\",
                    \"job_id\": \"$job_id\",
                    \"files_inbox\": \"\"
                },
                {
                    \"parent\": \"\",
                    \"content\": \"WebSockets sound like what I need. Could you show me an example of setting up a WebSocket connection in a React component?\",
                    \"job_id\": \"$job_id\",
                    \"files_inbox\": \"\"
                },
                {
                    \"parent\": \"\",
                    \"content\": \"Here's a basic example using the useEffect hook to manage a WebSocket connection:\\n\\nconst [data, setData] = useState(null);\\n\\nuseEffect(() => {\\n  const ws = new WebSocket('ws://your-server-url');\\n  \\n  ws.onmessage = (event) => {\\n    setData(JSON.parse(event.data));\\n  };\\n\\n  return () => ws.close();\\n}, []);\\n\\nThis sets up a connection when the component mounts and cleanly closes it on unmount. Would you like me to explain more about error handling and reconnection strategies?\",
                    \"job_id\": \"$job_id\",
                    \"files_inbox\": \"\"
                }
            ]
        }"

    echo "Added messages to job: $job_id"
    echo "----------------------------------------"
}

echo "Creating $JOBS_COUNT jobs..."

for ((i=1; i<=$JOBS_COUNT; i++)); do
    echo "Processing job $i of $JOBS_COUNT"
    create_job_and_add_messages
done

echo "Completed creating $JOBS_COUNT jobs" 