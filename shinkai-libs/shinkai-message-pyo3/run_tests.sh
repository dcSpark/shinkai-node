#!/bin/bash

# Activate your virtual environment
source ./venv/bin/activate

# Run maturin develop and capture its output
output=$(maturin develop)

# If maturin develop is successful, extract the path of the built wheel file
if [ $? -eq 0 ]; then
    wheel_file=$(echo "$output" | grep -o '/.*\.whl')
    
    # Update the installed package using the built wheel file
    pip install --upgrade "$wheel_file"
    
    # Run the tests
    python3 -m unittest tests.test_shinkai_message_pyo3
else
    echo "maturin develop failed"
fi