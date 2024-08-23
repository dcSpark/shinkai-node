use std::{cmp::Ordering, collections::HashMap};

use polars::{frame::DataFrame, prelude::NamedFrom, series::Series};

use crate::models::{Entity, Relationship};

pub fn get_in_network_relationships(
    selected_entities: &Vec<Entity>,
    relationships: &Vec<Relationship>,
    ranking_attribute: &str,
) -> Vec<Relationship> {
    let selected_entity_names: Vec<String> = selected_entities.iter().map(|entity| entity.title.clone()).collect();

    let selected_relationships: Vec<Relationship> = relationships
        .clone()
        .into_iter()
        .filter(|relationship| {
            selected_entity_names.contains(&relationship.source) && selected_entity_names.contains(&relationship.target)
        })
        .collect();

    if selected_relationships.len() <= 1 {
        return selected_relationships;
    }

    // Sort by ranking attribute
    sort_relationships_by_ranking_attribute(selected_relationships, selected_entities.to_vec(), ranking_attribute)
}

pub fn get_out_network_relationships(
    selected_entities: &Vec<Entity>,
    relationships: &Vec<Relationship>,
    ranking_attribute: &str,
) -> Vec<Relationship> {
    let selected_entity_names: Vec<String> = selected_entities.iter().map(|e| e.title.clone()).collect();

    let source_relationships: Vec<Relationship> = relationships
        .iter()
        .filter(|r| selected_entity_names.contains(&r.source) && !selected_entity_names.contains(&r.target))
        .cloned()
        .collect();

    let target_relationships: Vec<Relationship> = relationships
        .iter()
        .filter(|r| selected_entity_names.contains(&r.target) && !selected_entity_names.contains(&r.source))
        .cloned()
        .collect();

    let selected_relationships = [source_relationships, target_relationships].concat();

    sort_relationships_by_ranking_attribute(selected_relationships, selected_entities.to_vec(), ranking_attribute)
}

pub fn get_candidate_relationships(
    selected_entities: &Vec<Entity>,
    relationships: &Vec<Relationship>,
) -> Vec<Relationship> {
    let selected_entity_names: Vec<String> = selected_entities.iter().map(|entity| entity.title.clone()).collect();

    relationships
        .iter()
        .cloned()
        .filter(|relationship| {
            selected_entity_names.contains(&relationship.source) || selected_entity_names.contains(&relationship.target)
        })
        .collect()
}

pub fn get_entities_from_relationships(relationships: &Vec<Relationship>, entities: &Vec<Entity>) -> Vec<Entity> {
    let selected_entity_names: Vec<String> = relationships
        .iter()
        .flat_map(|relationship| vec![relationship.source.clone(), relationship.target.clone()])
        .collect();

    entities
        .iter()
        .cloned()
        .filter(|entity| selected_entity_names.contains(&entity.title))
        .collect()
}

pub fn sort_relationships_by_ranking_attribute(
    relationships: Vec<Relationship>,
    entities: Vec<Entity>,
    ranking_attribute: &str,
) -> Vec<Relationship> {
    if relationships.is_empty() {
        return relationships;
    }

    let mut relationships = relationships;

    let attribute_names: Vec<String> = if let Some(attributes) = &relationships[0].attributes {
        attributes.keys().cloned().collect()
    } else {
        Vec::new()
    };

    if attribute_names.contains(&ranking_attribute.to_string()) {
        relationships.sort_by(|a, b| {
            let a_rank = a
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(ranking_attribute))
                .and_then(|rank| rank.parse::<i32>().ok())
                .unwrap_or(0);
            let b_rank = b
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(ranking_attribute))
                .and_then(|rank| rank.parse::<i32>().ok())
                .unwrap_or(0);
            b_rank.cmp(&a_rank)
        });
    } else if ranking_attribute == "weight" {
        relationships.sort_by(|a, b| {
            let a_weight = a.weight.unwrap_or(0.0);
            let b_weight = b.weight.unwrap_or(0.0);
            b_weight.partial_cmp(&a_weight).unwrap_or(Ordering::Equal)
        });
    } else {
        relationships = calculate_relationship_combined_rank(relationships, entities, ranking_attribute);
        relationships.sort_by(|a, b| {
            let a_rank = a
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(ranking_attribute))
                .and_then(|rank| rank.parse::<i32>().ok())
                .unwrap_or(0);
            let b_rank = b
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(ranking_attribute))
                .and_then(|rank| rank.parse::<i32>().ok())
                .unwrap_or(0);
            b_rank.cmp(&a_rank)
        });
    }

    relationships
}

pub fn calculate_relationship_combined_rank(
    relationships: Vec<Relationship>,
    entities: Vec<Entity>,
    ranking_attribute: &str,
) -> Vec<Relationship> {
    let mut relationships = relationships;
    let entity_mappings: HashMap<_, _> = entities.iter().map(|e| (e.title.clone(), e)).collect();

    for relationship in relationships.iter_mut() {
        if relationship.attributes.is_none() {
            relationship.attributes = Some(HashMap::new());
        }

        let source_rank = entity_mappings
            .get(&relationship.source)
            .and_then(|e| e.rank)
            .unwrap_or(0);
        let target_rank = entity_mappings
            .get(&relationship.target)
            .and_then(|e| e.rank)
            .unwrap_or(0);

        if let Some(attributes) = &mut relationship.attributes {
            attributes.insert(ranking_attribute.to_string(), (source_rank + target_rank).to_string());
        }
    }

    relationships
}

pub fn to_relationship_dataframe(
    relationships: &Vec<Relationship>,
    include_relationship_weight: bool,
) -> anyhow::Result<DataFrame> {
    if relationships.is_empty() {
        return Ok(DataFrame::default());
    }

    let mut header = vec![
        "id".to_string(),
        "source".to_string(),
        "target".to_string(),
        "description".to_string(),
    ];

    if include_relationship_weight {
        header.push("weight".to_string());
    }

    let attribute_cols = if let Some(relationship) = relationships.first().cloned() {
        relationship
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

    let mut records = HashMap::new();

    for rel in relationships {
        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(rel.short_id.clone().unwrap_or_default());
        records
            .entry("source")
            .or_insert_with(Vec::new)
            .push(rel.source.clone());
        records
            .entry("target")
            .or_insert_with(Vec::new)
            .push(rel.target.clone());
        records
            .entry("description")
            .or_insert_with(Vec::new)
            .push(rel.description.clone().unwrap_or_default());

        if include_relationship_weight {
            records
                .entry("weight")
                .or_insert_with(Vec::new)
                .push(rel.weight.map(|r| r.to_string()).unwrap_or_default());
        }

        for field in &attribute_cols {
            records.entry(field).or_insert_with(Vec::new).push(
                rel.attributes
                    .as_ref()
                    .and_then(|attrs| attrs.get(field))
                    .cloned()
                    .unwrap_or_default(),
            );
        }
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

    Ok(record_df)
}
