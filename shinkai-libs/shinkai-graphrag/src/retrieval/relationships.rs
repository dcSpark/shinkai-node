use std::{cmp::Ordering, collections::HashMap};

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
