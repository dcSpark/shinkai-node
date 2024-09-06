use std::collections::{HashMap, HashSet};

use polars::frame::DataFrame;
use polars::prelude::NamedFrom;
use polars::series::Series;
use rand::prelude::SliceRandom;
use rand::{rngs::StdRng, SeedableRng};

use crate::models::{Entity, Relationship, TextUnit};

pub fn build_text_unit_context(
    text_units: Vec<TextUnit>,
    num_tokens_fn: fn(&str) -> usize,
    column_delimiter: &str,
    shuffle_data: bool,
    max_tokens: usize,
    context_name: &str,
    random_state: u64,
) -> anyhow::Result<(String, HashMap<String, DataFrame>)> {
    if text_units.is_empty() {
        return Ok((String::new(), HashMap::new()));
    }

    let mut text_units = text_units;

    let mut unique_ids = HashSet::new();
    text_units.retain(|unit| unique_ids.insert(unit.id.clone()));

    if shuffle_data {
        let mut rng = StdRng::seed_from_u64(random_state);
        text_units.shuffle(&mut rng);
    }

    let mut current_context_text = format!("-----{}-----\n", context_name);
    let mut header = vec!["id".to_string(), "text".to_string()];

    let attribute_cols = if let Some(text_unit) = text_units.first().cloned() {
        text_unit
            .attributes
            .unwrap_or_default()
            .keys().cloned()
            .collect::<Vec<String>>()
    } else {
        Vec::new()
    };

    let attribute_cols: Vec<String> = attribute_cols.into_iter().filter(|col| !header.contains(col)).collect();
    header.extend(attribute_cols.clone());
    current_context_text += &header.join(column_delimiter);

    let mut current_tokens = num_tokens_fn(&current_context_text);
    let mut records = HashMap::new();

    for unit in text_units {
        let mut new_context = vec![unit.short_id.clone().unwrap_or_default(), unit.text.clone()];

        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(unit.short_id.unwrap_or_default());
        records.entry("text").or_insert_with(Vec::new).push(unit.text);

        for field in &attribute_cols {
            let field_value = unit
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(field))
                .cloned()
                .unwrap_or_default();
            new_context.push(field_value);

            records.entry(field).or_insert_with(Vec::new).push(
                unit.attributes
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
        let series = Series::new(header, data_values);
        data_series.push(series);
    }

    let record_df = if !data_series.is_empty() {
        DataFrame::new(data_series)?
    } else {
        DataFrame::default()
    };

    Ok((
        current_context_text,
        HashMap::from([(context_name.to_lowercase(), record_df)]),
    ))
}

pub fn count_relationships(
    text_unit: &TextUnit,
    entity: &Entity,
    relationships: &HashMap<String, Relationship>,
) -> usize {
    let matching_relationships: Vec<&Relationship> = if text_unit.relationship_ids.is_none() {
        let entity_relationships: Vec<&Relationship> = relationships
            .values()
            .filter(|rel| rel.source == entity.title || rel.target == entity.title)
            .collect();

        let entity_relationships: Vec<&Relationship> = entity_relationships
            .into_iter()
            .filter(|rel| rel.text_unit_ids.is_some())
            .collect();

        entity_relationships
            .into_iter()
            .filter(|rel| rel.text_unit_ids.as_ref().unwrap().contains(&text_unit.id))
            .collect()
    } else {
        let text_unit_relationships: Vec<&Relationship> = text_unit
            .relationship_ids
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|rel_id| relationships.get(rel_id))
            .collect();

        text_unit_relationships
            .into_iter()
            .filter(|rel| rel.source == entity.title || rel.target == entity.title)
            .collect()
    };

    matching_relationships.len()
}
