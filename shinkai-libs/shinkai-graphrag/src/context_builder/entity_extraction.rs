use std::collections::HashSet;

use crate::{
    input::retrieval::entities::{get_entity_by_key, get_entity_by_name},
    llm::base::BaseTextEmbedding,
    models::Entity,
    vector_stores::{lancedb::LanceDBVectorStore, vector_store::VectorStore},
};

pub async fn map_query_to_entities(
    query: &str,
    text_embedding_vectorstore: &LanceDBVectorStore,
    text_embedder: &(dyn BaseTextEmbedding + Send + Sync),
    all_entities: &Vec<Entity>,
    embedding_vectorstore_key: &str,
    include_entity_names: Option<Vec<String>>,
    exclude_entity_names: Option<Vec<String>>,
    k: usize,
    oversample_scaler: usize,
) -> anyhow::Result<Vec<Entity>> {
    let include_entity_names = include_entity_names.unwrap_or_default();
    let exclude_entity_names: HashSet<String> = exclude_entity_names.unwrap_or_default().into_iter().collect();
    let mut matched_entities = Vec::new();

    if !query.is_empty() {
        let search_results = text_embedding_vectorstore
            .similarity_search_by_text(query, text_embedder, k * oversample_scaler)
            .await?;

        for result in search_results {
            if let Some(matched) = get_entity_by_key(all_entities, embedding_vectorstore_key, &result.document.id) {
                matched_entities.push(matched);
            }
        }
    } else {
        let mut all_entities = all_entities.clone();
        all_entities.sort_by(|a, b| b.rank.unwrap_or(0).cmp(&a.rank.unwrap_or(0)));
        matched_entities = all_entities.iter().take(k).cloned().collect();
    }

    matched_entities.retain(|entity| !exclude_entity_names.contains(&entity.title));

    let mut included_entities = Vec::new();
    for entity_name in include_entity_names {
        included_entities.extend(get_entity_by_name(all_entities, &entity_name));
    }

    included_entities.extend(matched_entities);
    Ok(included_entities)
}
