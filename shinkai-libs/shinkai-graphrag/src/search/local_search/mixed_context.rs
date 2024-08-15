use std::collections::HashMap;

use polars::frame::DataFrame;

use crate::{
    llm::llm::BaseTextEmbedding,
    models::{CommunityReport, Entity, Relationship, TextUnit},
    vector_stores::lancedb::LanceDBVectorStore,
};

#[derive(Debug, Clone)]
pub struct LocalSearchContextBuilderParams {
    pub query: String,
    pub include_entity_names: Option<Vec<String>>,
    pub exclude_entity_names: Option<Vec<String>>,
    pub max_tokens: i32,
    pub text_unit_prop: f32,
    pub community_prop: f32,
    pub top_k_mapped_entities: i32,
    pub top_k_relationships: i32,
    pub include_community_rank: bool,
    pub include_entity_rank: bool,
    pub rank_description: String,
    pub include_relationship_weight: bool,
    pub relationship_ranking_attribute: String,
    pub return_candidate_context: bool,
    pub use_community_summary: bool,
    pub min_community_rank: i32,
    pub community_context_name: String,
    pub column_delimiter: String,
    // pub conversation_history: Option<ConversationHistory>,
    // pub conversation_history_max_turns: Option<i32>,
    // pub conversation_history_user_turns_only: bool,
}

pub fn default_local_context_params() -> LocalSearchContextBuilderParams {
    LocalSearchContextBuilderParams {
        query: String::new(),
        include_entity_names: None,
        exclude_entity_names: None,
        max_tokens: 8000,
        text_unit_prop: 0.5,
        community_prop: 0.25,
        top_k_mapped_entities: 10,
        top_k_relationships: 10,
        include_community_rank: false,
        include_entity_rank: false,
        rank_description: "number of relationships".to_string(),
        include_relationship_weight: false,
        relationship_ranking_attribute: "rank".to_string(),
        return_candidate_context: false,
        use_community_summary: false,
        min_community_rank: 0,
        community_context_name: "Reports".to_string(),
        column_delimiter: "|".to_string(),
    }
}

pub struct LocalSearchMixedContext {
    entities: HashMap<String, Entity>,
    entity_text_embeddings: LanceDBVectorStore,
    text_embedder: Box<dyn BaseTextEmbedding>,
    text_units: HashMap<String, TextUnit>,
    community_reports: HashMap<String, CommunityReport>,
    relationships: HashMap<String, Relationship>,
    num_tokens_fn: fn(&str) -> usize,
    embedding_vectorstore_key: String,
}

impl LocalSearchMixedContext {
    pub fn new(
        entities: Vec<Entity>,
        entity_text_embeddings: LanceDBVectorStore,
        text_embedder: Box<dyn BaseTextEmbedding>,
        text_units: Option<Vec<TextUnit>>,
        community_reports: Option<Vec<CommunityReport>>,
        relationships: Option<Vec<Relationship>>,
        num_tokens_fn: fn(&str) -> usize,
        embedding_vectorstore_key: String,
    ) -> Self {
        let mut context = LocalSearchMixedContext {
            entities: HashMap::new(),
            entity_text_embeddings,
            text_embedder,
            text_units: HashMap::new(),
            community_reports: HashMap::new(),
            relationships: HashMap::new(),
            num_tokens_fn,
            embedding_vectorstore_key,
        };

        for entity in entities {
            context.entities.insert(entity.id.clone(), entity);
        }

        if let Some(units) = text_units {
            for unit in units {
                context.text_units.insert(unit.id.clone(), unit);
            }
        }

        if let Some(reports) = community_reports {
            for report in reports {
                context.community_reports.insert(report.id.clone(), report);
            }
        }

        if let Some(relations) = relationships {
            for relation in relations {
                context.relationships.insert(relation.id.clone(), relation);
            }
        }

        context
    }

    pub async fn build_context(
        &self,
        context_builder_params: LocalSearchContextBuilderParams,
    ) -> anyhow::Result<(String, HashMap<String, DataFrame>)> {
        let LocalSearchContextBuilderParams {
            query,
            include_entity_names,
            exclude_entity_names,
            max_tokens,
            text_unit_prop,
            community_prop,
            top_k_mapped_entities,
            top_k_relationships,
            include_community_rank,
            include_entity_rank,
            rank_description,
            include_relationship_weight,
            relationship_ranking_attribute,
            return_candidate_context,
            use_community_summary,
            min_community_rank,
            community_context_name,
            column_delimiter,
        } = context_builder_params;

        let include_entity_names = include_entity_names.unwrap_or_default();
        let exclude_entity_names = exclude_entity_names.unwrap_or_default();

        if community_prop + text_unit_prop > 1.0 {
            return Err(anyhow::anyhow!(
                "The sum of community_prop and text_unit_prop must be less than or equal to 1.0"
            ));
        }

        let context_text = String::new();
        let context_records = HashMap::new();
        Ok((context_text, context_records))
    }
}
