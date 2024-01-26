#!/bin/bash

# Create a virtual environment if it doesn't exist
if [ ! -d "./venv" ]
then
    python3 -m venv venv
fi

# Activate your virtual environment
source ./venv/bin/activate

# Delete the existing .whl file
rm ./target/wheels/shinkai_message_pyo3*.whl

# Run maturin develop and capture its output
output=$(maturin build -i python3)

# If maturin develop is successful, extract the path of the built wheel file
if [ $? -eq 0 ]; then
    # Find the wheel file that starts with "shinkai_message_pyo3" in the "./target/wheels/" directory
    wheel_file=$(find ./target/wheels/ -name 'shinkai_message_pyo3*.whl' -print -quit)
     
    # Update the installed package using the built wheel file
    pip install --upgrade --force-reinstall "$wheel_file"
    
    # Run the tests
    python3 -m unittest tests.test_shinkai_message_pyo3
else
    echo "maturin develop failed"
fi
