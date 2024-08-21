use std::collections::HashSet;
use uuid::Uuid;

use crate::{llm::llm::BaseTextEmbedding, models::Entity, vector_stores::vector_store::VectorStore};

pub fn map_query_to_entities(
    query: &str,
    text_embedding_vectorstore: &Box<dyn VectorStore>,
    text_embedder: &Box<dyn BaseTextEmbedding>,
    all_entities: &Vec<Entity>,
    embedding_vectorstore_key: &str,
    include_entity_names: Option<Vec<String>>,
    exclude_entity_names: Option<Vec<String>>,
    k: usize,
    oversample_scaler: usize,
) -> Vec<Entity> {
    let include_entity_names = include_entity_names.unwrap_or_else(Vec::new);
    let exclude_entity_names: HashSet<String> = exclude_entity_names.unwrap_or_else(Vec::new).into_iter().collect();
    let mut matched_entities = Vec::new();

    if !query.is_empty() {
        let search_results = text_embedding_vectorstore.similarity_search_by_text(
            query,
            &|t| text_embedder.embed(t),
            k * oversample_scaler,
        );

        for result in search_results {
            if let Some(matched) = get_entity_by_key(all_entities, &embedding_vectorstore_key, &result.document.id) {
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
    included_entities
}

pub fn get_entity_by_key(entities: &Vec<Entity>, key: &str, value: &str) -> Option<Entity> {
    for entity in entities {
        match key {
            "id" => {
                if entity.id == value
                    || is_valid_uuid(value) && entity.id == Uuid::parse_str(value).unwrap().to_string().replace("-", "")
                {
                    return Some(entity.clone());
                }
            }
            "short_id" => {
                if entity.short_id.as_ref().unwrap_or(&"".to_string()) == value
                    || is_valid_uuid(value)
                        && entity.short_id.as_ref().unwrap_or(&"".to_string())
                            == Uuid::parse_str(value).unwrap().to_string().replace("-", "").as_str()
                {
                    return Some(entity.clone());
                }
            }
            "title" => {
                if entity.title == value {
                    return Some(entity.clone());
                }
            }
            "entity_type" => {
                if entity.entity_type.as_ref().unwrap_or(&"".to_string()) == value {
                    return Some(entity.clone());
                }
            }
            "description" => {
                if entity.description.as_ref().unwrap_or(&"".to_string()) == value {
                    return Some(entity.clone());
                }
            }
            _ => {}
        }
    }
    None
}

pub fn get_entity_by_name(entities: &Vec<Entity>, entity_name: &str) -> Vec<Entity> {
    entities
        .iter()
        .filter(|entity| entity.title == entity_name)
        .cloned()
        .collect()
}

pub fn is_valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
