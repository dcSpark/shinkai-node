// use std::collections::HashMap;
// use tokio::sync::RwLock;
// use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
// use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
// use crate::db::ShinkaiDB;
// use crate::vector_fs::vector_fs::VectorFS;
// use crate::tools::shinkai_tool::ShinkaiTool;

// pub struct WorkflowManager {
//     pub internal: RwLock<HashMap<ShinkaiName, ShinkaiTool>>,
//     pub embedding_generator: RemoteEmbeddingGenerator,
//     pub db: ShinkaiDB,
//     pub vector_fs: VectorFS,
// }

// impl WorkflowManager {
//     pub fn new(
//         embedding_generator: RemoteEmbeddingGenerator,
//         db: ShinkaiDB,
//         vector_fs: VectorFS,
//     ) -> Self {
//         WorkflowManager {
//             internal: RwLock::new(HashMap::new()),
//             embedding_generator,
//             db,
//             vector_fs,
//         }
//     }
// }