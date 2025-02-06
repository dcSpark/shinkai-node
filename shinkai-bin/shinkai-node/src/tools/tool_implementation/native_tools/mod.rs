// Otra tarea es el tool para el knowledge

// Para esto del tool, aquí hay un ejemplo:
// https://github.com/dcSpark/shinkai-node/blob/main/shinkai-bin/shinkai-node/src/tools/tool_implementation/sql_processor.rs
// Hay que crear una herramienta igual, en un archivo igual que ese.
// Solo tiene que mantener la firma del run

pub mod config_setup;
pub mod llm_map_reduce_processor;
pub mod llm_prompt_processor;
pub mod sql_processor;
pub mod tool_knowledge;
pub mod typescript_unsafe_processor;
