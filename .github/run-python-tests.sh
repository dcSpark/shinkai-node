#!/bin/bash
cd /app/shinkai-libs/shinkai-message-pyo3

# Initialize pyenv
set -e
export PYENV_ROOT="$HOME/.pyenv"
export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init --path)"
eval "$(pyenv init -)"

# Get the path of the Python interpreter inside the Docker container
python_path=$(which python)

# Set the PYO3_PYTHON and PYTHON_SYS_EXECUTABLE environment variables
export PYO3_PYTHON="$python_path"
export PYTHON_SYS_EXECUTABLE="$python_path"

# Print the Python version
python --version

# Create a virtual environment if it doesn't exist
if [ ! -d "./venv" ]
then
    python -m venv venv
fi

# Activate your virtual environment
source ./venv/bin/activate

# Run maturin develop and capture its output
output=$(maturin build -i python)

# If maturin develop is successful, extract the path of the built wheel file
if [ $? -eq 0 ]; then
    echo "Maturin build successful"
    wheel_file=$(ls target/wheels/*.whl)
    echo "Wheel file: $wheel_file"
    
    # Update the installed package using the built wheel file
    echo "Running pip install --upgrade \"$wheel_file\"..."
    pip_output=$(pip install --upgrade "$wheel_file")
    
    # Run the tests and print their output
    echo "Running tests..."
    test_output=$(python -m unittest tests.test_shinkai_message_pyo3)
    echo "$test_output"
else
    echo "maturin develop failed"
fi