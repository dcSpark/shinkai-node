use std::collections::HashMap;

use polars::prelude::*;
use polars_lazy::dsl::col;
use serde::{Deserialize, Serialize};

use super::indexer_reports::filter_under_community_level;

pub fn read_indexer_entities(
    final_nodes: &DataFrame,
    final_entities: &DataFrame,
    community_level: u32,
) -> anyhow::Result<Vec<Entity>> {
    let entity_df = final_nodes.clone();
    let mut entity_df = filter_under_community_level(&entity_df, community_level)?;

    let entity_df = entity_df.rename("title", "name")?.rename("degree", "rank")?;

    let entity_df = entity_df
        .clone()
        .lazy()
        .with_column(col("community").fill_null(lit(-1)))
        .collect()?;
    let entity_df = entity_df
        .clone()
        .lazy()
        .with_column(col("community").cast(DataType::Int32))
        .collect()?;
    let entity_df = entity_df
        .clone()
        .lazy()
        .with_column(col("rank").cast(DataType::Int32))
        .collect()?;

    let entity_embedding_df = final_entities.clone();

    let entity_df = entity_df
        .clone()
        .lazy()
        .group_by([col("name"), col("rank")])
        .agg([col("community").max()])
        .collect()?;

    let entity_df = entity_df
        .clone()
        .lazy()
        .with_column(col("community").cast(DataType::String))
        .collect()?;

    let entity_df = entity_df
        .clone()
        .lazy()
        .join(
            entity_embedding_df.clone().lazy(),
            [col("name")],
            [col("name")],
            JoinArgs::new(JoinType::Inner),
        )
        .collect()?;

    let entity_df = entity_df
        .clone()
        .lazy()
        .filter(len().over([col("name")]).gt(lit(1)))
        .collect()?;

    let entities = read_entities(
        &entity_df,
        "id",
        Some("human_readable_id"),
        "name",
        Some("type"),
        Some("description"),
        None,
        Some("description_embedding"),
        None,
        Some("community"),
        Some("text_unit_ids"),
        None,
        Some("rank"),
    )?;

    Ok(entities)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entity {
    pub id: String,
    pub short_id: Option<String>,
    pub title: String,
    pub entity_type: Option<String>,
    pub description: Option<String>,
    pub description_embedding: Option<Vec<f64>>,
    pub name_embedding: Option<Vec<f64>>,
    pub graph_embedding: Option<Vec<f64>>,
    pub community_ids: Option<Vec<String>>,
    pub text_unit_ids: Option<Vec<String>>,
    pub document_ids: Option<Vec<String>>,
    pub rank: Option<i32>,
    pub attributes: Option<HashMap<String, String>>,
}

pub fn read_entities(
    df: &DataFrame,
    id_col: &str,
    short_id_col: Option<&str>,
    title_col: &str,
    type_col: Option<&str>,
    description_col: Option<&str>,
    name_embedding_col: Option<&str>,
    description_embedding_col: Option<&str>,
    graph_embedding_col: Option<&str>,
    community_col: Option<&str>,
    text_unit_ids_col: Option<&str>,
    document_ids_col: Option<&str>,
    rank_col: Option<&str>,
    // attributes_cols: Option<Vec<&str>>,
) -> anyhow::Result<Vec<Entity>> {
    let column_names = [
        id_col,
        short_id_col.unwrap_or("short_id"),
        title_col,
        type_col.unwrap_or("type"),
        description_col.unwrap_or("description"),
        name_embedding_col.unwrap_or("name_embedding"),
        description_embedding_col.unwrap_or("description_embedding"),
        graph_embedding_col.unwrap_or("graph_embedding"),
        community_col.unwrap_or("community_ids"),
        text_unit_ids_col.unwrap_or("text_unit_ids"),
        document_ids_col.unwrap_or("document_ids"),
        rank_col.unwrap_or("degree"),
    ];

    let mut df = df.clone();
    df.as_single_chunk_par();
    let mut iters = df.columns(column_names)?.iter().map(|s| s.iter()).collect::<Vec<_>>();

    let mut rows = Vec::new();
    for _row in 0..df.height() {
        let mut row_values = Vec::new();
        for iter in &mut iters {
            let value = iter.next();
            if let Some(value) = value {
                row_values.push(value.to_string());
            }
        }
        rows.push(row_values);
    }

    let mut entities = Vec::new();
    for row in rows {
        let report = Entity {
            id: row.get(0).unwrap_or(&String::new()).to_string(),
            short_id: Some(row.get(1).unwrap_or(&String::new()).to_string()),
            title: row.get(2).unwrap_or(&String::new()).to_string(),
            entity_type: Some(row.get(3).unwrap_or(&String::new()).to_string()),
            description: Some(row.get(4).unwrap_or(&String::new()).to_string()),
            name_embedding: Some(
                row.get(5)
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|v| v.parse::<f64>().unwrap_or(0.0))
                    .collect(),
            ),
            description_embedding: Some(
                row.get(6)
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|v| v.parse::<f64>().unwrap_or(0.0))
                    .collect(),
            ),
            graph_embedding: Some(
                row.get(7)
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|v| v.parse::<f64>().unwrap_or(0.0))
                    .collect(),
            ),
            community_ids: Some(
                row.get(8)
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|v| v.to_string())
                    .collect(),
            ),
            text_unit_ids: Some(
                row.get(9)
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|v| v.to_string())
                    .collect(),
            ),
            document_ids: Some(
                row.get(10)
                    .unwrap_or(&String::new())
                    .split(',')
                    .map(|v| v.to_string())
                    .collect(),
            ),
            rank: Some(row.get(11).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0)),
            attributes: None,
        };
        entities.push(report);
    }

    Ok(entities)
}
