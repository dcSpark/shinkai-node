# Generic Inference Chain

The `GenericInferenceChain` is a core component responsible for handling inference tasks in the Shinkai system. It implements the `InferenceChain` trait and provides a flexible mechanism for processing user messages, performing vector searches, and executing tool calls.

## Overview

The chain operates in a sequential manner, following these main steps:

1. Vector search for knowledge
2. Tool selection
3. Prompt generation
4. LLM inference
5. Function call handling (if required)
6. Response formatting

## Key Components

### Main Structures

- `GenericInferenceChain`: The main struct containing:
  - `context`: An `InferenceChainContext` holding job-related information
  - `ws_manager_trait`: Optional WebSocket manager for real-time updates

### Core Functions

#### `start_chain`

The main entry point that orchestrates the entire inference process. It handles:

1. **Knowledge Search**
   - Performs vector search if the scope isn't empty
   - Merges agent scope files if using an agent provider
   - Searches through provided file paths and folders

2. **Tool Selection**
   - Handles two cases:
     a. User explicitly selected tool
     b. Automatic tool selection based on capabilities
   - Considers:
     - Streaming configuration
     - Tool permissions
     - Provider capabilities
     - Agent-specific tools

3. **Prompt Generation**
   - Uses `JobPromptGenerator::generic_inference_prompt`
   - Supports custom prompts from job config or agent config
   - Incorporates:
     - System prompts
     - User messages
     - Image files
     - Vector search results
     - Tool information
     - Job history

4. **Inference Loop**
   - Manages iterations with LLM provider
   - Handles function calls
   - Processes tool responses
   - Updates WebSocket clients

### Supporting Functions

#### `JobPromptGenerator::generic_inference_prompt`

Generates structured prompts for the LLM with the following features:
- Adds system prompts with priority handling
- Incorporates job step history
- Manages tool information with decreasing priority
- Handles vector search results
- Supports custom user and system prompts
- Manages image files and function call responses

```rust
pub fn generic_inference_prompt(
    custom_system_prompt: Option<String>,
    custom_user_prompt: Option<String>,
    user_message: String,
    image_files: HashMap<String, String>,
    ret_nodes: ShinkaiFileChunkCollection,
    summary_text: Option<String>,
    job_step_history: Option<Vec<ShinkaiMessage>>,
    tools: Vec<ShinkaiTool>,
    function_call: Option<ToolCallFunctionResponse>,
    job_id: String,
    node_env: NodeEnvironment,
) -> Prompt
```

#### `JobManager::search_for_chunks_in_resources`

Performs vector search across resources with the following capabilities:
- Searches through file paths and folders
- Manages token limits for context windows
- Handles embedding generation and comparison
- Expands search results with neighboring chunks
- Optimizes token usage within max_tokens_in_prompt

```rust
pub async fn search_for_chunks_in_resources(
    fs_files_paths: Vec<ShinkaiPath>,
    fs_folder_paths: Vec<ShinkaiPath>,
    job_filenames: Vec<String>,
    job_id: String,
    scope: &MinimalJobScope,
    sqlite_manager: Arc<SqliteManager>,
    query_text: String,
    num_of_top_results: usize,
    max_tokens_in_prompt: usize,
    embedding_generator: RemoteEmbeddingGenerator,
) -> Result<ShinkaiFileChunkCollection, SqliteManagerError>
```

#### `JobManager::inference_with_llm_provider`

Manages LLM interactions with the following features:
- Handles streaming responses
- Supports WebSocket updates
- Manages LLM stopping mechanism
- Processes job configurations
- Handles provider/agent selection

```rust
pub async fn inference_with_llm_provider(
    llm_provider: ProviderOrAgent,
    filled_prompt: Prompt,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    config: Option<JobConfig>,
    llm_stopper: Arc<LLMStopper>,
    db: Arc<SqliteManager>,
) -> Result<LLMInferenceResponse, LLMProviderError>
```

#### `trigger_ws_update`

Handles WebSocket updates for tool execution:
- Updates tool execution status
- Manages tool metadata
- Handles function call results
- Provides real-time progress updates

## Error Handling

The chain includes comprehensive error handling for:
- Maximum iterations exceeded
- LLM service limits
- Function call failures
- Tool retrieval errors
- WebSocket communication issues

## WebSocket Integration

Real-time updates are provided through WebSocket connections, including:
- Tool execution status
- Function call results
- Progress updates
- LLM response streaming

## Dependencies

Key dependencies and managers:
- `SqliteManager`: Database operations
- `ToolRouter`: Tool management and execution
- `SheetManager`: Sheet-related operations
- `JobCallbackManager`: Callback handling
- `LLMStopper`: Inference control
- `RemoteEmbeddingGenerator`: Embedding generation

## Usage Example

```rust
let chain = GenericInferenceChain::new(
    context,
    ws_manager_trait,
);

let result = chain.run_chain().await?;
```

## Configuration

The chain can be configured through:
- Job configuration
- Agent configuration
- System environment settings
- Tool capabilities
- LLM provider settings

## Best Practices

1. Always set appropriate iteration limits
2. Handle WebSocket updates appropriately
3. Implement proper error handling
4. Monitor tool execution status
5. Validate function calls before execution
6. Manage token limits for context windows
7. Optimize vector searches for performance
8. Handle streaming responses efficiently 