use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use polars::{frame::DataFrame, prelude::NamedFrom, series::Series};

use crate::{
    models::{Entity, Relationship},
    retrieval::{
        entities::to_entity_dataframe,
        relationships::{
            get_candidate_relationships, get_entities_from_relationships, get_in_network_relationships,
            get_out_network_relationships, to_relationship_dataframe,
        },
    },
};

pub fn build_entity_context(
    selected_entities: Vec<Entity>,
    num_tokens_fn: fn(&str) -> usize,
    max_tokens: usize,
    include_entity_rank: bool,
    rank_description: &str,
    column_delimiter: &str,
    context_name: &str,
) -> anyhow::Result<(String, DataFrame)> {
    if selected_entities.is_empty() {
        return Ok((String::new(), DataFrame::default()));
    }

    let mut current_context_text = format!("-----{}-----\n", context_name);
    let mut header = vec!["id".to_string(), "entity".to_string(), "description".to_string()];

    if include_entity_rank {
        header.push(rank_description.to_string());
    }

    let attribute_cols = if let Some(first_entity) = selected_entities.first().cloned() {
        first_entity
            .attributes
            .unwrap_or_default()
            .keys()
            .map(|s| s.clone())
            .collect::<Vec<String>>()
    } else {
        Vec::new()
    };

    header.extend(attribute_cols.clone());
    current_context_text += &header.join(column_delimiter);

    let mut current_tokens = num_tokens_fn(&current_context_text);
    let mut records = HashMap::new();

    for entity in selected_entities {
        let mut new_context = vec![
            entity.short_id.clone().unwrap_or_default(),
            entity.title.clone(),
            entity.description.clone().unwrap_or_default(),
        ];

        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(entity.short_id.unwrap_or_default());
        records.entry("entity").or_insert_with(Vec::new).push(entity.title);
        records
            .entry("description")
            .or_insert_with(Vec::new)
            .push(entity.description.unwrap_or_default());

        if include_entity_rank {
            new_context.push(entity.rank.unwrap_or(0).to_string());

            records
                .entry("rank")
                .or_insert_with(Vec::new)
                .push(entity.rank.map(|r| r.to_string()).unwrap_or_default());
        }

        for field in &attribute_cols {
            let field_value = entity
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(field))
                .cloned()
                .unwrap_or_default();
            new_context.push(field_value);

            records.entry(field).or_insert_with(Vec::new).push(
                entity
                    .attributes
                    .as_ref()
                    .and_then(|attrs| attrs.get(field))
                    .cloned()
                    .unwrap_or_default(),
            );
        }

        let new_context_text = new_context.join(column_delimiter);
        let new_tokens = num_tokens_fn(&new_context_text);

        if current_tokens + new_tokens > max_tokens {
            break;
        }

        current_context_text += &format!("\n{}", new_context_text);
        current_tokens += new_tokens;
    }

    let mut data_series = Vec::new();
    for (header, data_values) in records {
        if header == "rank" {
            let data_values = data_values
                .iter()
                .map(|v| v.parse::<f64>().unwrap_or(0.0))
                .collect::<Vec<_>>();
            let series = Series::new(header, data_values);
            data_series.push(series);
        } else {
            let series = Series::new(header, data_values);
            data_series.push(series);
        };
    }

    let record_df = if !data_series.is_empty() {
        DataFrame::new(data_series)?
    } else {
        DataFrame::default()
    };

    Ok((current_context_text, record_df))
}

pub fn build_relationship_context(
    selected_entities: &Vec<Entity>,
    relationships: &Vec<Relationship>,
    num_tokens_fn: fn(&str) -> usize,
    include_relationship_weight: bool,
    max_tokens: usize,
    top_k_relationships: usize,
    relationship_ranking_attribute: &str,
    column_delimiter: &str,
    context_name: &str,
) -> anyhow::Result<(String, DataFrame)> {
    // Filter relationships based on the criteria
    let selected_relationships = _filter_relationships(
        &selected_entities,
        &relationships,
        top_k_relationships,
        relationship_ranking_attribute,
    );

    if selected_entities.is_empty() || selected_relationships.is_empty() {
        return Ok((String::new(), DataFrame::default()));
    }

    let mut current_context_text = format!("-----{}-----\n", context_name);
    let mut header = vec![
        "id".to_string(),
        "source".to_string(),
        "target".to_string(),
        "description".to_string(),
    ];

    if include_relationship_weight {
        header.push("weight".to_string());
    }

    let attribute_cols = if let Some(first_rel) = selected_relationships.first().cloned() {
        first_rel
            .attributes
            .unwrap_or_default()
            .keys()
            .map(|s| s.clone())
            .collect::<Vec<String>>()
    } else {
        Vec::new()
    };

    let attribute_cols: Vec<String> = attribute_cols.into_iter().filter(|col| !header.contains(col)).collect();
    header.extend(attribute_cols.clone());

    current_context_text.push_str(&header.join(column_delimiter));
    current_context_text.push('\n');

    let mut current_tokens = num_tokens_fn(&current_context_text);
    let mut records = HashMap::new();

    for rel in selected_relationships {
        let mut new_context = vec![
            rel.short_id.clone().unwrap_or_default(),
            rel.source.clone(),
            rel.target.clone(),
            rel.description.clone().unwrap_or_default(),
        ];

        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(rel.short_id.unwrap_or_default());
        records.entry("source").or_insert_with(Vec::new).push(rel.source);
        records.entry("target").or_insert_with(Vec::new).push(rel.target);
        records
            .entry("description")
            .or_insert_with(Vec::new)
            .push(rel.description.unwrap_or_default());

        if include_relationship_weight {
            new_context.push(rel.weight.map_or(String::new(), |w| w.to_string()));

            records
                .entry("weight")
                .or_insert_with(Vec::new)
                .push(rel.weight.map(|r| r.to_string()).unwrap_or_default());
        }

        for field in &attribute_cols {
            let field_value = rel
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(field))
                .cloned()
                .unwrap_or_default();
            new_context.push(field_value);

            records.entry(field).or_insert_with(Vec::new).push(
                rel.attributes
                    .as_ref()
                    .and_then(|attrs| attrs.get(field))
                    .cloned()
                    .unwrap_or_default(),
            );
        }

        let mut new_context_text = new_context.join(column_delimiter);
        new_context_text.push('\n');
        let new_tokens = num_tokens_fn(&new_context_text);

        if current_tokens + new_tokens > max_tokens {
            break;
        }

        current_context_text += new_context_text.as_str();
        current_tokens += new_tokens;
    }

    let mut data_series = Vec::new();
    for (header, data_values) in records {
        if header == "weight" {
            let data_values = data_values
                .iter()
                .map(|v| v.parse::<f64>().unwrap_or(0.0))
                .collect::<Vec<_>>();
            let series = Series::new(header, data_values);
            data_series.push(series);
        } else {
            let series = Series::new(header, data_values);
            data_series.push(series);
        };
    }

    let record_df = if !data_series.is_empty() {
        DataFrame::new(data_series)?
    } else {
        DataFrame::default()
    };

    Ok((current_context_text, record_df))
}

fn _filter_relationships(
    selected_entities: &Vec<Entity>,
    relationships: &Vec<Relationship>,
    top_k_relationships: usize,
    relationship_ranking_attribute: &str,
) -> Vec<Relationship> {
    // First priority: in-network relationships (i.e. relationships between selected entities)
    let in_network_relationships =
        get_in_network_relationships(selected_entities, relationships, relationship_ranking_attribute);

    // Second priority -  out-of-network relationships
    // (i.e. relationships between selected entities and other entities that are not within the selected entities)
    let mut out_network_relationships =
        get_out_network_relationships(selected_entities, relationships, relationship_ranking_attribute);

    if out_network_relationships.len() <= 1 {
        return [in_network_relationships, out_network_relationships].concat();
    }

    // within out-of-network relationships, prioritize mutual relationships
    // (i.e. relationships with out-network entities that are shared with multiple selected entities)
    let selected_entity_names: HashSet<String> = selected_entities.iter().map(|e| e.title.clone()).collect();

    let out_network_source_names: Vec<String> = out_network_relationships
        .iter()
        .filter(|r| !selected_entity_names.contains(&r.source))
        .map(|r| r.source.clone())
        .collect();

    let out_network_target_names: Vec<String> = out_network_relationships
        .iter()
        .filter(|r| !selected_entity_names.contains(&r.target))
        .map(|r| r.target.clone())
        .collect();

    let out_network_entity_names: HashSet<String> = out_network_source_names
        .into_iter()
        .chain(out_network_target_names.into_iter())
        .collect();

    let mut out_network_entity_links: HashMap<String, usize> = HashMap::new();

    for entity_name in out_network_entity_names {
        let targets: HashSet<String> = out_network_relationships
            .iter()
            .filter(|r| r.source == entity_name)
            .map(|r| r.target.clone())
            .collect();

        let sources: HashSet<String> = out_network_relationships
            .iter()
            .filter(|r| r.target == entity_name)
            .map(|r| r.source.clone())
            .collect();

        out_network_entity_links.insert(entity_name, targets.union(&sources).count());
    }

    // sort out-network relationships by number of links and rank_attributes
    for relationship in &mut out_network_relationships {
        if relationship.attributes.is_none() {
            relationship.attributes = Some(HashMap::new());
        }

        let links = if out_network_entity_links.contains_key(&relationship.source) {
            *out_network_entity_links.get(&relationship.source).unwrap()
        } else {
            *out_network_entity_links.get(&relationship.target).unwrap()
        };
        relationship
            .attributes
            .as_mut()
            .unwrap()
            .insert("links".to_string(), links.to_string());
    }

    // Sort by attributes[links] first, then by ranking_attribute
    if relationship_ranking_attribute == "weight" {
        out_network_relationships.sort_by(|a, b| {
            let a_links = a
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get("links"))
                .and_then(|rank| rank.parse::<usize>().ok())
                .unwrap_or(0);
            let b_links = b
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get("links"))
                .and_then(|rank| rank.parse::<usize>().ok())
                .unwrap_or(0);

            b_links
                .partial_cmp(&a_links)
                .unwrap_or(Ordering::Equal)
                .then(b.weight.partial_cmp(&a.weight).unwrap_or(Ordering::Equal))
        });
    } else {
        out_network_relationships.sort_by(|a, b| {
            let a_links = a
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get("links"))
                .and_then(|rank| rank.parse::<usize>().ok())
                .unwrap_or(0);
            let b_links = b
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get("links"))
                .and_then(|rank| rank.parse::<usize>().ok())
                .unwrap_or(0);

            let a_rank = a
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(relationship_ranking_attribute))
                .and_then(|rank| rank.parse::<f64>().ok())
                .unwrap_or(0.0);
            let b_rank = b
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(relationship_ranking_attribute))
                .and_then(|rank| rank.parse::<f64>().ok())
                .unwrap_or(0.0);

            b_links
                .partial_cmp(&a_links)
                .unwrap_or(Ordering::Equal)
                .then(b_rank.partial_cmp(&a_rank).unwrap_or(Ordering::Equal))
        });
    }

    let relationship_budget = top_k_relationships * selected_entities.len();
    out_network_relationships.truncate(relationship_budget);

    Vec::new()
}

pub fn get_candidate_context(
    selected_entities: &Vec<Entity>,
    entities: &Vec<Entity>,
    relationships: &Vec<Relationship>,
    include_entity_rank: bool,
    entity_rank_description: &str,
    include_relationship_weight: bool,
) -> anyhow::Result<HashMap<String, DataFrame>> {
    let mut candidate_context = HashMap::new();

    let candidate_relationships = get_candidate_relationships(selected_entities, relationships);
    candidate_context.insert(
        "relationships".to_string(),
        to_relationship_dataframe(&candidate_relationships, include_relationship_weight)?,
    );

    let candidate_entities = get_entities_from_relationships(&candidate_relationships, entities);
    candidate_context.insert(
        "entities".to_string(),
        to_entity_dataframe(&candidate_entities, include_entity_rank, entity_rank_description)?,
    );

    Ok(candidate_context)
}
