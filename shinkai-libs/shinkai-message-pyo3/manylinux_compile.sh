#!/bin/bash

# Define container name for easier reference
CONTAINER_NAME="shinkai_build_container"

# Start the Docker container, mapping the volume
sudo docker run -it -d --name $CONTAINER_NAME -v /home/nico/ai-dcspark/development/shinkai-node/shinkai-libs:/project pyo3-maturin:latest /bin/bash

# Run the commands inside the Docker container to generate the file
sudo docker exec -it $CONTAINER_NAME bash -c "cd shinkai-message-pyo3 && rm -rf target && maturin build -i python"

# Check for the existence of the file every 10 seconds, but timeout after 10 minutes
timeout=$((10*60)) # 10 minutes in seconds
interval=10        # 10 seconds interval

file_exists=0
elapsed_time=0
latest_file=""

while [[ $file_exists -eq 0 && $elapsed_time -lt $timeout ]]; do
    # Check if the file exists in the Docker container
    latest_file=$(sudo docker exec $CONTAINER_NAME bash -c "ls -t /project/shinkai-message-pyo3/target/wheels/ | grep 'shinkai_message_pyo3' | head -n1")
    if [[ -n $latest_file ]]; then
        file_exists=1
    else
        sleep $interval
        elapsed_time=$((elapsed_time + interval))
    fi
done

# If the file exists, copy it to the host. Otherwise, print an error message
if [[ $file_exists -eq 1 ]]; then
    sudo docker cp $CONTAINER_NAME:/project/shinkai-message-pyo3/target/wheels/$latest_file /home/nico/ai-dcspark/development/shinkai-node/shinkai-libs/
    echo "File has been copied successfully!"
else
    echo "Error: File was not created within the time limit."
fi

# Stop and remove the container
sudo docker stop $CONTAINER_NAME
sudo docker rm $CONTAINER_NAME