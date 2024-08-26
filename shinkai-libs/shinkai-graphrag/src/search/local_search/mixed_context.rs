use std::collections::HashMap;

use polars::{
    frame::DataFrame,
    prelude::{is_in, NamedFrom},
    series::Series,
};

use crate::{
    context_builder::{
        community_context::CommunityContext,
        entity_extraction::map_query_to_entities,
        local_context::{build_entity_context, build_relationship_context, get_candidate_context},
        source_context::{build_text_unit_context, count_relationships},
    },
    input::retrieval::{community_reports::get_candidate_communities, text_units::get_candidate_text_units},
    llm::llm::BaseTextEmbedding,
    models::{CommunityReport, Entity, Relationship, TextUnit},
    vector_stores::vector_store::VectorStore,
};

#[derive(Debug, Clone)]
pub struct MixedContextBuilderParams {
    pub query: String,
    pub include_entity_names: Option<Vec<String>>,
    pub exclude_entity_names: Option<Vec<String>>,
    pub max_tokens: usize,
    pub text_unit_prop: f32,
    pub community_prop: f32,
    pub top_k_mapped_entities: usize,
    pub top_k_relationships: usize,
    pub include_community_rank: bool,
    pub include_entity_rank: bool,
    pub rank_description: String,
    pub include_relationship_weight: bool,
    pub relationship_ranking_attribute: String,
    pub return_candidate_context: bool,
    pub use_community_summary: bool,
    pub min_community_rank: u32,
    pub community_context_name: String,
    pub column_delimiter: String,
    // pub conversation_history: Option<ConversationHistory>,
    // pub conversation_history_max_turns: Option<i32>,
    // pub conversation_history_user_turns_only: bool,
}

pub fn default_local_context_params() -> MixedContextBuilderParams {
    MixedContextBuilderParams {
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
    entity_text_embeddings: Box<dyn VectorStore>,
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
        entity_text_embeddings: Box<dyn VectorStore>,
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
        context_builder_params: MixedContextBuilderParams,
    ) -> anyhow::Result<(String, HashMap<String, DataFrame>)> {
        let MixedContextBuilderParams {
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

        let selected_entities = map_query_to_entities(
            &query,
            &self.entity_text_embeddings,
            &self.text_embedder,
            &self.entities.values().cloned().collect::<Vec<Entity>>(),
            &self.embedding_vectorstore_key,
            Some(include_entity_names),
            Some(exclude_entity_names),
            top_k_mapped_entities,
            2,
        );

        let mut final_context = Vec::new();
        let mut final_context_data = HashMap::new();

        // build community context
        let community_tokens = std::cmp::max((max_tokens as f32 * community_prop) as usize, 0);
        let (community_context, community_context_data) = self._build_community_context(
            selected_entities.clone(),
            community_tokens,
            use_community_summary,
            &column_delimiter,
            include_community_rank,
            min_community_rank,
            return_candidate_context,
            &community_context_name,
        )?;

        if !community_context.trim().is_empty() {
            final_context.push(community_context);
            final_context_data.extend(community_context_data);
        }

        // build local (i.e. entity-relationship-covariate) context
        let local_prop = 1 as f32 - community_prop - text_unit_prop;
        let local_tokens = std::cmp::max((max_tokens as f32 * local_prop) as usize, 0);
        let (local_context, local_context_data) = self._build_local_context(
            selected_entities.clone(),
            local_tokens,
            include_entity_rank,
            &rank_description,
            include_relationship_weight,
            top_k_relationships,
            &relationship_ranking_attribute,
            return_candidate_context,
            &column_delimiter,
        )?;

        if !local_context.trim().is_empty() {
            final_context.push(local_context);
            final_context_data.extend(local_context_data);
        }

        // build text unit context
        let text_unit_tokens = std::cmp::max((max_tokens as f32 * text_unit_prop) as usize, 0);
        let (text_unit_context, text_unit_context_data) = self._build_text_unit_context(
            selected_entities.clone(),
            text_unit_tokens,
            return_candidate_context,
            "|",
            "Sources",
        )?;

        if !text_unit_context.trim().is_empty() {
            final_context.push(text_unit_context);
            final_context_data.extend(text_unit_context_data);
        }

        Ok((final_context.join("\n\n"), final_context_data))
    }

    fn _build_community_context(
        &self,
        selected_entities: Vec<Entity>,
        max_tokens: usize,
        use_community_summary: bool,
        column_delimiter: &str,
        include_community_rank: bool,
        min_community_rank: u32,
        return_candidate_context: bool,
        context_name: &str,
    ) -> anyhow::Result<(String, HashMap<String, DataFrame>)> {
        if selected_entities.is_empty() || self.community_reports.is_empty() {
            return Ok((
                "".to_string(),
                HashMap::from([(context_name.to_lowercase(), DataFrame::default())]),
            ));
        }

        let mut community_matches: HashMap<String, usize> = HashMap::new();
        for entity in &selected_entities {
            if let Some(community_ids) = &entity.community_ids {
                for community_id in community_ids {
                    *community_matches.entry(community_id.to_string()).or_insert(0) += 1;
                }
            }
        }

        let mut selected_communities: Vec<CommunityReport> = Vec::new();
        for community_id in community_matches.keys() {
            if let Some(community) = self.community_reports.get(community_id) {
                selected_communities.push(community.clone());
            }
        }

        for community in &mut selected_communities {
            if community.attributes.is_none() {
                community.attributes = Some(HashMap::new());
            }
            if let Some(attributes) = &mut community.attributes {
                attributes.insert("matches".to_string(), community_matches[&community.id].to_string());
            }
        }

        selected_communities.sort_by(|a, b| {
            let a_matches = a
                .attributes
                .as_ref()
                .unwrap()
                .get("matches")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let b_matches = b
                .attributes
                .as_ref()
                .unwrap()
                .get("matches")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let a_rank = a.rank.unwrap();
            let b_rank = b.rank.unwrap();
            (b_matches, b_rank).partial_cmp(&(a_matches, a_rank)).unwrap()
        });

        for community in &mut selected_communities {
            if let Some(attributes) = &mut community.attributes {
                attributes.remove("matches");
            }
        }

        let (context_text, context_data) = CommunityContext::build_community_context(
            selected_communities,
            None,
            self.num_tokens_fn,
            use_community_summary,
            column_delimiter,
            false,
            include_community_rank,
            min_community_rank,
            "rank",
            true,
            "occurrence weight",
            true,
            max_tokens,
            true,
            context_name,
        )?;

        let mut context_text_result = "".to_string();
        if !context_text.is_empty() {
            context_text_result = context_text.join("\n\n");
        }

        let mut context_data = context_data;
        if return_candidate_context {
            let candidate_context_data = get_candidate_communities(
                selected_entities,
                self.community_reports.values().cloned().collect(),
                use_community_summary,
                include_community_rank,
            )?;

            let context_key = context_name.to_lowercase();
            if !context_data.contains_key(&context_key) {
                let mut new_data = candidate_context_data.clone();
                new_data.with_column(Series::new("in_context", vec![false; candidate_context_data.height()]))?;
                context_data.insert(context_key.to_string(), new_data);
            } else {
                let existing_data = context_data.get(&context_key).unwrap();
                if candidate_context_data
                    .get_column_names()
                    .contains(&"id".to_string().as_str())
                    && existing_data.get_column_names().contains(&"id".to_string().as_str())
                {
                    let existing_ids = existing_data.column("id")?;
                    let context_ids = candidate_context_data.column("id")?;
                    let mut new_data = candidate_context_data.clone();
                    let in_context = is_in(context_ids, existing_ids)?;
                    let in_context = Series::new("in_context", in_context);
                    new_data.with_column(in_context)?;
                    context_data.insert(context_key.to_string(), new_data);
                } else {
                    let mut existing_data = existing_data.clone();
                    existing_data.with_column(Series::new("in_context", vec![true; existing_data.height()]))?;
                    context_data.insert(context_key.to_string(), existing_data);
                }
            }
        }

        Ok((context_text_result, context_data))
    }

    fn _build_local_context(
        &self,
        selected_entities: Vec<Entity>,
        max_tokens: usize,
        include_entity_rank: bool,
        rank_description: &str,
        include_relationship_weight: bool,
        top_k_relationships: usize,
        relationship_ranking_attribute: &str,
        return_candidate_context: bool,
        column_delimiter: &str,
    ) -> anyhow::Result<(String, HashMap<String, DataFrame>)> {
        let (entity_context, entity_context_data) = build_entity_context(
            selected_entities.clone(),
            self.num_tokens_fn,
            max_tokens,
            include_entity_rank,
            rank_description,
            column_delimiter,
            "Entities",
        )?;

        let entity_tokens = (self.num_tokens_fn)(&entity_context);

        let mut added_entities = Vec::new();
        let mut final_context = Vec::new();
        let mut final_context_data = HashMap::new();

        for entity in &selected_entities {
            let mut current_context = Vec::new();
            let mut current_context_data = HashMap::new();
            added_entities.push(entity.clone());

            let (relationship_context, relationship_context_data) = build_relationship_context(
                &added_entities,
                &self.relationships.values().cloned().collect(),
                self.num_tokens_fn,
                include_relationship_weight,
                max_tokens,
                top_k_relationships,
                relationship_ranking_attribute,
                column_delimiter,
                "Relationships",
            )?;

            current_context.push(relationship_context.clone());
            current_context_data.insert("relationships".to_string(), relationship_context_data);

            let total_tokens = entity_tokens + (self.num_tokens_fn)(&relationship_context);

            if total_tokens > max_tokens {
                eprintln!("Reached token limit - reverting to previous context state");
                break;
            }

            final_context = current_context;
            final_context_data = current_context_data;
        }

        let mut final_context_text = entity_context.to_string();
        final_context_text.push_str("\n\n");
        final_context_text.push_str(&final_context.join("\n\n"));
        final_context_data.insert("entities".to_string(), entity_context_data.clone());

        if return_candidate_context {
            let entities = self.entities.values().cloned().collect();
            let relationships = self.relationships.values().cloned().collect();

            let candidate_context_data = get_candidate_context(
                &selected_entities,
                &entities,
                &relationships,
                include_entity_rank,
                rank_description,
                include_relationship_weight,
            )?;

            for (key, candidate_df) in candidate_context_data {
                if !final_context_data.contains_key(&key) {
                    final_context_data.insert(key.clone(), candidate_df);
                } else {
                    let in_context_df = final_context_data.get_mut(&key).unwrap();

                    if in_context_df.get_column_names().contains(&"id".to_string().as_str())
                        && candidate_df.get_column_names().contains(&"id".to_string().as_str())
                    {
                        let context_ids = in_context_df.column("id")?;
                        let candidate_ids = candidate_df.column("id")?;
                        let mut new_data = candidate_df.clone();
                        let in_context = is_in(candidate_ids, context_ids)?;
                        let in_context = Series::new("in_context", in_context);
                        new_data.with_column(in_context)?;
                        final_context_data.insert(key.clone(), new_data);
                    } else {
                        in_context_df.with_column(Series::new("in_context", vec![true; in_context_df.height()]))?;
                    }
                }
            }
        } else {
            for (_key, context_df) in final_context_data.iter_mut() {
                context_df.with_column(Series::new("in_context", vec![true; context_df.height()]))?;
            }
        }

        Ok((final_context_text, final_context_data))
    }

    fn _build_text_unit_context(
        &self,
        selected_entities: Vec<Entity>,
        max_tokens: usize,
        return_candidate_context: bool,
        column_delimiter: &str,
        context_name: &str,
    ) -> anyhow::Result<(String, HashMap<String, DataFrame>)> {
        if selected_entities.is_empty() || self.text_units.is_empty() {
            return Ok((String::new(), HashMap::new()));
        }

        let mut selected_text_units: Vec<TextUnit> = Vec::new();

        for (index, entity) in selected_entities.iter().enumerate() {
            if let Some(text_unit_ids) = &entity.text_unit_ids {
                for text_id in text_unit_ids {
                    if !selected_text_units.iter().any(|unit| &unit.id == text_id)
                        && self.text_units.contains_key(text_id)
                    {
                        let mut selected_unit = self.text_units[text_id].clone();
                        let num_relationships = count_relationships(&selected_unit, entity, &self.relationships);
                        selected_unit
                            .attributes
                            .as_mut()
                            .unwrap_or(&mut HashMap::new())
                            .insert("entity_order".to_string(), index.to_string());
                        selected_unit
                            .attributes
                            .as_mut()
                            .unwrap_or(&mut HashMap::new())
                            .insert("num_relationships".to_string(), num_relationships.to_string());
                        selected_text_units.push(selected_unit);
                    }
                }
            }
        }

        selected_text_units.sort_by(|a, b| {
            let a_order = a
                .attributes
                .as_ref()
                .unwrap()
                .get("entity_order")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let b_order = b
                .attributes
                .as_ref()
                .unwrap()
                .get("entity_order")
                .unwrap()
                .parse::<usize>()
                .unwrap();

            let a_relationships = a
                .attributes
                .as_ref()
                .unwrap()
                .get("num_relationships")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let b_relationships = b
                .attributes
                .as_ref()
                .unwrap()
                .get("num_relationships")
                .unwrap()
                .parse::<usize>()
                .unwrap();

            a_order
                .cmp(&b_order)
                .then_with(|| b_relationships.cmp(&a_relationships))
        });

        for unit in &mut selected_text_units {
            unit.attributes.as_mut().unwrap().remove("entity_order");
            unit.attributes.as_mut().unwrap().remove("num_relationships");
        }

        let (context_text, context_data) = build_text_unit_context(
            selected_text_units,
            self.num_tokens_fn,
            column_delimiter,
            false,
            max_tokens,
            context_name,
            86,
        )?;

        let mut context_data = context_data;
        if return_candidate_context {
            let candidate_context_data =
                get_candidate_text_units(&selected_entities, &self.text_units.values().cloned().collect())?;

            let context_key = context_name.to_lowercase();
            if !context_data.contains_key(&context_key) {
                let mut new_data = candidate_context_data.clone();
                new_data.with_column(Series::new("in_context", vec![false; candidate_context_data.height()]))?;
                context_data.insert(context_key.to_string(), new_data);
            } else {
                let existing_data = context_data.get(&context_key).unwrap();
                if candidate_context_data
                    .get_column_names()
                    .contains(&"id".to_string().as_str())
                    && existing_data.get_column_names().contains(&"id".to_string().as_str())
                {
                    let existing_ids = existing_data.column("id")?;
                    let context_ids = candidate_context_data.column("id")?;
                    let mut new_data = candidate_context_data.clone();
                    let in_context = is_in(context_ids, existing_ids)?;
                    let in_context = Series::new("in_context", in_context);
                    new_data.with_column(in_context)?;
                    context_data.insert(context_key.to_string(), new_data);
                } else {
                    let mut existing_data = existing_data.clone();
                    existing_data.with_column(Series::new("in_context", vec![true; existing_data.height()]))?;
                    context_data.insert(context_key.to_string(), existing_data);
                }
            }
        }

        Ok((context_text, context_data))
    }
}
