use std::collections::HashSet;

use polars::prelude::*;
use polars_lazy::dsl::col;

use crate::models::Entity;

use super::indexer_reports::filter_under_community_level;

pub fn read_indexer_entities(
    final_nodes: &DataFrame,
    final_entities: &DataFrame,
    community_level: u32,
) -> anyhow::Result<Vec<Entity>> {
    let entity_df = final_nodes.clone();
    let entity_df = filter_under_community_level(&entity_df, community_level)?;

    let entity_embedding_df = final_entities.clone();

    let entity_df = entity_df
        .lazy()
        .rename(["title", "degree"], ["name", "rank"])
        .with_column(col("community").fill_null(lit(-1)))
        .with_column(col("community").cast(DataType::Int32))
        .with_column(col("rank").cast(DataType::Int32))
        .group_by([col("name"), col("rank")])
        .agg([col("community").max()])
        .with_column(col("community").cast(DataType::String))
        .join(
            entity_embedding_df.lazy(),
            [col("name")],
            [col("name")],
            JoinArgs::new(JoinType::Inner),
        )
        .collect()?;

    let entities = read_entities(
        entity_df,
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

pub fn read_entities(
    df: DataFrame,
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
        Some(id_col),
        short_id_col,
        Some(title_col),
        type_col,
        description_col,
        name_embedding_col,
        description_embedding_col,
        graph_embedding_col,
        community_col,
        text_unit_ids_col,
        document_ids_col,
        rank_col,
    ]
    .iter()
    .filter_map(|&v| v.map(|v| v.to_string()))
    .collect::<Vec<_>>();

    let column_names = column_names.into_iter().collect::<HashSet<String>>().into_vec();

    let mut df = df;
    df.as_single_chunk_par();
    let mut iters = df
        .columns(column_names.clone())?
        .iter()
        .map(|s| s.iter())
        .collect::<Vec<_>>();

    let mut rows = Vec::new();
    for _row in 0..df.height() {
        let mut row_values = Vec::new();
        for iter in &mut iters {
            let value = iter.next();
            if let Some(value) = value {
                row_values.push(value);
            }
        }
        rows.push(row_values);
    }

    let mut entities = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        let report = Entity {
            id: get_field(&row, id_col, &column_names)
                .map(|id| id.to_string())
                .unwrap_or(String::new()),
            short_id: Some(
                short_id_col
                    .map(|short_id| get_field(&row, short_id, &column_names))
                    .flatten()
                    .map(|short_id| short_id.to_string())
                    .unwrap_or(idx.to_string()),
            ),
            title: get_field(&row, title_col, &column_names)
                .map(|title| title.to_string())
                .unwrap_or(String::new()),
            entity_type: type_col
                .map(|type_col| get_field(&row, type_col, &column_names))
                .flatten()
                .map(|entity_type| entity_type.to_string()),
            description: description_col
                .map(|description_col| get_field(&row, description_col, &column_names))
                .flatten()
                .map(|description| description.to_string()),
            name_embedding: name_embedding_col.map(|name_embedding_col| {
                get_field(&row, name_embedding_col, &column_names)
                    .map(|name_embedding| match name_embedding {
                        AnyValue::List(series) => series
                            .f64()
                            .unwrap_or(&ChunkedArray::from_vec(name_embedding_col, vec![]))
                            .iter()
                            .map(|v| v.unwrap_or(0.0))
                            .collect::<Vec<f64>>(),
                        value => vec![value.to_string().parse::<f64>().unwrap_or(0.0)],
                    })
                    .unwrap_or_else(|| Vec::new())
            }),
            description_embedding: description_embedding_col.map(|description_embedding_col| {
                get_field(&row, description_embedding_col, &column_names)
                    .map(|description_embedding| match description_embedding {
                        AnyValue::List(series) => series
                            .f64()
                            .unwrap_or(&ChunkedArray::from_vec(description_embedding_col, vec![]))
                            .iter()
                            .map(|v| v.unwrap_or(0.0))
                            .collect::<Vec<f64>>(),
                        value => vec![value.to_string().parse::<f64>().unwrap_or(0.0)],
                    })
                    .unwrap_or_else(|| Vec::new())
            }),
            graph_embedding: graph_embedding_col.map(|graph_embedding_col| {
                get_field(&row, graph_embedding_col, &column_names)
                    .map(|graph_embedding| match graph_embedding {
                        AnyValue::List(series) => series
                            .f64()
                            .unwrap_or(&ChunkedArray::from_vec(graph_embedding_col, vec![]))
                            .iter()
                            .map(|v| v.unwrap_or(0.0))
                            .collect::<Vec<f64>>(),
                        value => vec![value.to_string().parse::<f64>().unwrap_or(0.0)],
                    })
                    .unwrap_or_else(|| Vec::new())
            }),
            community_ids: community_col.map(|community_col| {
                get_field(&row, community_col, &column_names)
                    .map(|community_ids| match community_ids {
                        AnyValue::List(series) => series
                            .str()
                            .unwrap_or(&StringChunked::default())
                            .iter()
                            .map(|v| v.unwrap_or("").to_string())
                            .collect::<Vec<String>>(),
                        value => vec![value.to_string()],
                    })
                    .unwrap_or_else(|| Vec::new())
            }),
            text_unit_ids: text_unit_ids_col.map(|text_unit_ids_col| {
                get_field(&row, text_unit_ids_col, &column_names)
                    .map(|text_unit_ids| match text_unit_ids {
                        AnyValue::List(series) => series
                            .str()
                            .unwrap_or(&StringChunked::default())
                            .iter()
                            .map(|v| v.unwrap_or("").to_string())
                            .collect::<Vec<String>>(),
                        value => vec![value.to_string()],
                    })
                    .unwrap_or_else(|| Vec::new())
            }),
            document_ids: document_ids_col.map(|document_ids_col| {
                get_field(&row, document_ids_col, &column_names)
                    .map(|document_ids| match document_ids {
                        AnyValue::List(series) => series
                            .str()
                            .unwrap_or(&StringChunked::default())
                            .iter()
                            .map(|v| v.unwrap_or("").to_string())
                            .collect::<Vec<String>>(),
                        value => vec![value.to_string()],
                    })
                    .unwrap_or_else(|| Vec::new())
            }),
            rank: rank_col
                .map(|rank_col| {
                    get_field(&row, rank_col, &column_names).map(|v| v.to_string().parse::<i32>().unwrap_or(0))
                })
                .flatten(),
            attributes: None,
        };
        entities.push(report);
    }

    let mut unique_entities: Vec<Entity> = Vec::new();
    let mut entity_ids: HashSet<String> = HashSet::new();

    for entity in entities {
        if !entity_ids.contains(&entity.id) {
            unique_entities.push(entity.clone());
            entity_ids.insert(entity.id);
        }
    }

    Ok(unique_entities)
}

pub fn get_field<'a>(
    row: &'a Vec<AnyValue<'a>>,
    column_name: &'a str,
    column_names: &'a Vec<String>,
) -> Option<AnyValue<'a>> {
    match column_names.iter().position(|x| x == column_name) {
        Some(index) => row.get(index).cloned(),
        None => None,
    }
}
