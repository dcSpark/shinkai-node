#!/usr/bin/env python3
"""
Regenerates the prompts_data.rs file with proper embeddings for the default embedding model.

This script:
1. Reads the existing prompts from prompts_data.rs
2. Gets the current default embedding model from environment or defaults to Embedding Gemma 300M
3. Generates new embeddings using Ollama API
4. Writes the updated prompts_data.rs file with correct dimensions

Usage:
    python3 scripts/regenerate_prompts_data.py [--model MODEL_NAME] [--ollama-url URL]
"""

import json
import subprocess
import re
import argparse
import sys
import os
from typing import List, Dict, Any

DEFAULT_MODEL = "embeddinggemma:300m"
DEFAULT_OLLAMA_URL = "http://localhost:11434"
PROMPTS_DATA_PATH = "shinkai-libs/shinkai-sqlite/src/files/prompts_data.rs"

class EmbeddingGenerator:
    def __init__(self, ollama_url: str, model: str):
        self.ollama_url = ollama_url.rstrip('/')
        self.model = model
    
    def generate_embedding(self, text: str) -> List[float]:
        """Generate embedding for the given text using Ollama API."""
        url = f"{self.ollama_url}/api/embeddings"
        payload = {
            "model": self.model,
            "prompt": text
        }
        
        try:
            # Use curl to make the HTTP request
            curl_cmd = [
                'curl',
                '-s',
                '-X', 'POST',
                '-H', 'Content-Type: application/json',
                '-d', json.dumps(payload),
                url
            ]
            
            result = subprocess.run(curl_cmd, capture_output=True, text=True, timeout=30)
            if result.returncode != 0:
                raise Exception(f"Curl failed with return code {result.returncode}: {result.stderr}")
            
            response_data = json.loads(result.stdout)
            return response_data["embedding"]
        except Exception as e:
            print(f"Error generating embedding: {e}")
            raise

def parse_prompts_from_rust_file(file_path: str) -> tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    """Parse prompts from the Rust prompts_data.rs file."""
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Extract PROMPTS_JSON_TESTING
    testing_match = re.search(r'pub static PROMPTS_JSON_TESTING: &str = r#"(\[.*?\])"#;', content, re.DOTALL)
    if not testing_match:
        raise ValueError("Could not find PROMPTS_JSON_TESTING in file")
    
    testing_json = testing_match.group(1)
    # Unescape the JSON by replacing \" with "
    testing_json = testing_json.replace('\\"', '"')
    try:
        testing_prompts = json.loads(testing_json)
    except json.JSONDecodeError as e:
        raise ValueError(f"Failed to parse PROMPTS_JSON_TESTING: {e}\nFirst 200 chars: {testing_json[:200]}")
    
    # Extract PROMPTS_JSON
    full_match = re.search(r'pub static PROMPTS_JSON: &str = r#"(\[.*?\])"#;', content, re.DOTALL)
    if not full_match:
        raise ValueError("Could not find PROMPTS_JSON in file")
    
    full_json = full_match.group(1)
    # Unescape the JSON by replacing \" with "
    full_json = full_json.replace('\\"', '"')
    try:
        full_prompts = json.loads(full_json)
    except json.JSONDecodeError as e:
        raise ValueError(f"Failed to parse PROMPTS_JSON: {e}\nFirst 200 chars: {full_json[:200]}")
    
    return testing_prompts, full_prompts

def regenerate_embeddings(prompts: List[Dict[str, Any]], generator: EmbeddingGenerator) -> List[Dict[str, Any]]:
    """Regenerate embeddings for all prompts."""
    updated_prompts = []
    
    for i, prompt in enumerate(prompts):
        print(f"Processing prompt {i+1}/{len(prompts)}: {prompt['name']}")
        
        # Generate new embedding
        new_embedding = generator.generate_embedding(prompt['prompt'])
        
        # Update the prompt with new embedding
        updated_prompt = prompt.copy()
        updated_prompt['embedding'] = new_embedding
        updated_prompts.append(updated_prompt)
    
    return updated_prompts

def format_rust_json(data: List[Dict[str, Any]]) -> str:
    """Format the data as a JSON string suitable for Rust raw string literal."""
    # Convert to JSON with proper formatting
    # No escaping needed for Rust raw string literals r#"..."#
    json_str = json.dumps(data, indent=2)
    return json_str

def write_prompts_data_file(testing_prompts: List[Dict[str, Any]], full_prompts: List[Dict[str, Any]], output_path: str):
    """Write the updated prompts to the Rust file."""
    testing_json = format_rust_json(testing_prompts)
    full_json = format_rust_json(full_prompts)
    
    content = f'''pub static PROMPTS_JSON_TESTING: &str = r#"{testing_json}"#;

pub static PROMPTS_JSON: &str = r#"{full_json}"#;
'''
    
    with open(output_path, 'w') as f:
        f.write(content)

def main():
    parser = argparse.ArgumentParser(description="Regenerate embeddings for static prompts")
    parser.add_argument("--model", default=DEFAULT_MODEL, help=f"Embedding model to use (default: {DEFAULT_MODEL})")
    parser.add_argument("--ollama-url", default=DEFAULT_OLLAMA_URL, help=f"Ollama API URL (default: {DEFAULT_OLLAMA_URL})")
    parser.add_argument("--dry-run", action="store_true", help="Just test the process without writing files")
    
    args = parser.parse_args()
    
    # Get model from environment if available
    env_model = os.getenv("DEFAULT_EMBEDDING_MODEL")
    if env_model:
        # Map environment model names to Ollama model names
        model_mapping = {
            "embeddinggemma:300m": "embeddinggemma:300m",
            "jina/jina-embeddings-v2-base-es:latest": "jina/jina-embeddings-v2-base-es:latest", 
            "snowflake-arctic-embed:xs": "snowflake-arctic-embed:xs"
        }
        args.model = model_mapping.get(env_model, args.model)
        print(f"Using model from environment: {args.model}")
    
    print(f"🚀 Starting embedding regeneration with model: {args.model}")
    print(f"📡 Ollama URL: {args.ollama_url}")
    
    # Initialize embedding generator
    generator = EmbeddingGenerator(args.ollama_url, args.model)
    
    # Test connection
    try:
        test_embedding = generator.generate_embedding("test")
        print(f"✅ Successfully connected to Ollama. Embedding dimension: {len(test_embedding)}")
    except Exception as e:
        print(f"❌ Failed to connect to Ollama: {e}")
        sys.exit(1)
    
    # Parse existing prompts
    try:
        testing_prompts, full_prompts = parse_prompts_from_rust_file(PROMPTS_DATA_PATH)
        print(f"📚 Loaded {len(testing_prompts)} testing prompts and {len(full_prompts)} full prompts")
    except Exception as e:
        print(f"❌ Failed to parse prompts file: {e}")
        sys.exit(1)
    
    if args.dry_run:
        print("🔍 Dry run mode - not writing files")
        return
    
    # Regenerate embeddings for testing prompts
    print("\n🔄 Regenerating embeddings for testing prompts...")
    updated_testing = regenerate_embeddings(testing_prompts, generator)
    
    # Regenerate embeddings for full prompts
    print("\n🔄 Regenerating embeddings for full prompts...")
    updated_full = regenerate_embeddings(full_prompts, generator)
    
    # Write updated file
    print(f"\n💾 Writing updated prompts to {PROMPTS_DATA_PATH}...")
    write_prompts_data_file(updated_testing, updated_full, PROMPTS_DATA_PATH)
    
    print(f"✅ Successfully regenerated embeddings!")
    print(f"📈 New embedding dimensions: {len(test_embedding)}")

if __name__ == "__main__":
    main()
