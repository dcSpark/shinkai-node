use std::collections::{HashMap, HashSet};

use polars::{frame::DataFrame, prelude::NamedFrom, series::Series};

use crate::models::{Entity, TextUnit};

pub fn get_candidate_text_units(
    selected_entities: &Vec<Entity>,
    text_units: &Vec<TextUnit>,
) -> anyhow::Result<DataFrame> {
    let mut selected_text_ids: HashSet<String> = HashSet::new();

    for entity in selected_entities {
        if let Some(ids) = &entity.text_unit_ids {
            for id in ids {
                selected_text_ids.insert(id.to_string());
            }
        }
    }

    let selected_text_units: Vec<TextUnit> = text_units
        .iter()
        .cloned()
        .filter(|unit| selected_text_ids.contains(&unit.id))
        .collect();

    to_text_unit_dataframe(selected_text_units)
}

pub fn to_text_unit_dataframe(text_units: Vec<TextUnit>) -> anyhow::Result<DataFrame> {
    if text_units.is_empty() {
        return Ok(DataFrame::default());
    }

    let mut header = vec!["id".to_string(), "text".to_string()];

    let attribute_cols = if let Some(text_unit) = text_units.first().cloned() {
        text_unit
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

    for unit in text_units {
        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(unit.short_id.clone().unwrap_or_default());
        records.entry("text").or_insert_with(Vec::new).push(unit.text.clone());

        for field in &attribute_cols {
            records.entry(field).or_insert_with(Vec::new).push(
                unit.attributes
                    .as_ref()
                    .and_then(|attrs| attrs.get(field))
                    .cloned()
                    .unwrap_or_default(),
            );
        }
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

    Ok(record_df)
}
