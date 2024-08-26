use std::collections::HashMap;

use polars::{frame::DataFrame, prelude::NamedFrom, series::Series};
use uuid::Uuid;

use crate::models::Entity;

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

pub fn to_entity_dataframe(
    entities: &Vec<Entity>,
    include_entity_rank: bool,
    rank_description: &str,
) -> anyhow::Result<DataFrame> {
    if entities.is_empty() {
        return Ok(DataFrame::default());
    }

    let mut header = vec!["id".to_string(), "entity".to_string(), "description".to_string()];

    if include_entity_rank {
        header.push(rank_description.to_string());
    }

    let attribute_cols = if let Some(first_entity) = entities.first().cloned() {
        first_entity
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

    for entity in entities {
        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(entity.short_id.clone().unwrap_or_default());
        records
            .entry("entity")
            .or_insert_with(Vec::new)
            .push(entity.title.clone());
        records
            .entry("description")
            .or_insert_with(Vec::new)
            .push(entity.description.clone().unwrap_or_default());

        if include_entity_rank {
            records
                .entry("rank")
                .or_insert_with(Vec::new)
                .push(entity.rank.map(|r| r.to_string()).unwrap_or_default());
        }

        for field in &attribute_cols {
            records.entry(field).or_insert_with(Vec::new).push(
                entity
                    .attributes
                    .as_ref()
                    .and_then(|attrs| attrs.get(field))
                    .cloned()
                    .unwrap_or_default(),
            );
        }
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

    Ok(record_df)
}

pub fn is_valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
