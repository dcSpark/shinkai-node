#!/bin/bash

# Default values for environment variables
JOBS_COUNT=${JOBS_COUNT:-1}
AUTH_TOKEN=${AUTH_TOKEN:-"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ikpva"}
API_BASE_URL=${API_BASE_URL:-"http://127.0.0.1:9950"}

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

    # Add messages to the job
    curl -s -X "POST" "$API_BASE_URL/v2/add_messages_god_mode" \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H 'Content-Type: application/json; charset=utf-8' \
        -d "{
            \"job_id\": \"$job_id\",
            \"messages\": [
                {
                    \"parent\": \"\",
                    \"content\": \"I'm working on a React component that needs to handle real-time updates. What's the best approach?\",
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