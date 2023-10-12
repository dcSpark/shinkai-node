#!/bin/bash

# Create a virtual environment if it doesn't exist
if [ ! -d "./venv" ]
then
    python -m venv venv
fi

# Activate your virtual environment
source ./venv/bin/activate

# Run maturin develop and capture its output
maturin build -i python --compatibility linux --strip --release
