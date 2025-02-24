# Inference Chains Architecture

This directory contains detailed documentation about Shinkai's inference chain system, which is responsible for processing and executing inference tasks.

## Overview

Inference chains are the core components that handle the execution of user requests in Shinkai. Each chain type is specialized for different kinds of tasks while sharing common infrastructure for:

- Vector search and knowledge retrieval
- Tool management and execution
- Prompt generation and handling
- LLM interaction
- Real-time updates via WebSocket

## Available Chains

1. [Generic Chain](generic_chain.md)
   - The standard inference chain for general-purpose tasks
   - Handles tool selection, vector search, and LLM interactions
   - Supports both direct LLM providers and agents

2. Sheet UI Chain (Coming Soon)
   - Specialized chain for handling spreadsheet operations
   - Manages CSV data and table manipulations
   - Provides specific UI-related functionality

3. Custom Chain Development (Coming Soon)
   - Guide for developing new inference chains
   - Best practices and common patterns
   - Integration with existing infrastructure

## Common Components

The following components are shared across different chain implementations:

1. Context Management (Coming Soon)
   - Job context handling
   - State management
   - Configuration inheritance

2. Tool Integration (Coming Soon)
   - Tool discovery and selection
   - Function call handling
   - Response processing

3. Vector Search (Coming Soon)
   - Resource management
   - Embedding generation
   - Search optimization

4. WebSocket Communication (Coming Soon)
   - Real-time updates
   - Progress tracking
   - Error handling

## Development Guidelines

When working with or extending inference chains:

1. Follow the established patterns for:
   - Error handling
   - WebSocket updates
   - Resource management
   - Configuration handling

2. Ensure proper implementation of:
   - The `InferenceChain` trait
   - Context management
   - Tool integration
   - Vector search capabilities

3. Consider:
   - Performance implications
   - Memory management
   - Error recovery
   - User experience 