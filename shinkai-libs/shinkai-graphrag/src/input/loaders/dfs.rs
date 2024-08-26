use std::collections::{HashMap, HashSet};

use polars::{
    frame::DataFrame,
    prelude::{AnyValue, ChunkedArray, IntoVec, StringChunked},
};

use crate::{
    models::{CommunityReport, Entity},
    vector_stores::vector_store::{VectorStore, VectorStoreDocument},
};

pub fn store_entity_semantic_embeddings(
    entities: Vec<Entity>,
    mut vectorstore: Box<dyn VectorStore>,
) -> Box<dyn VectorStore> {
    let documents: Vec<VectorStoreDocument> = entities
        .into_iter()
        .map(|entity| {
            let mut attributes = HashMap::new();
            attributes.insert("title".to_string(), entity.title.clone());
            if let Some(entity_attributes) = entity.attributes {
                attributes.extend(entity_attributes);
            }

            VectorStoreDocument {
                id: entity.id,
                text: entity.description,
                vector: entity.description_embedding,
                attributes,
            }
        })
        .collect();

    vectorstore.load_documents(documents, true);
    vectorstore
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

pub fn read_community_reports(
    df: DataFrame,
    id_col: &str,
    short_id_col: Option<&str>,
    title_col: &str,
    community_col: &str,
    summary_col: &str,
    content_col: &str,
    rank_col: Option<&str>,
    _summary_embedding_col: Option<&str>,
    _content_embedding_col: Option<&str>,
    // attributes_cols: Option<&[&str]>,
) -> anyhow::Result<Vec<CommunityReport>> {
    let column_names = [
        Some(id_col),
        short_id_col,
        Some(title_col),
        Some(community_col),
        Some(summary_col),
        Some(content_col),
        rank_col,
    ]
    .iter()
    .filter_map(|&v| v.map(|v| v.to_string()))
    .collect::<Vec<_>>();

    let column_names: Vec<String> = column_names.into_iter().collect::<HashSet<String>>().into_vec();

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

    let mut reports = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        let report = CommunityReport {
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
            community_id: get_field(&row, community_col, &column_names)
                .map(|community| community.to_string())
                .unwrap_or(String::new()),
            summary: get_field(&row, summary_col, &column_names)
                .map(|summary| summary.to_string())
                .unwrap_or(String::new()),
            full_content: get_field(&row, content_col, &column_names)
                .map(|content| content.to_string())
                .unwrap_or(String::new()),
            rank: rank_col
                .map(|rank_col| {
                    get_field(&row, rank_col, &column_names).map(|v| v.to_string().parse::<f64>().unwrap_or(0.0))
                })
                .flatten(),
            summary_embedding: None,
            full_content_embedding: None,
            attributes: None,
        };
        reports.push(report);
    }

    let mut unique_reports: Vec<CommunityReport> = Vec::new();
    let mut report_ids: HashSet<String> = HashSet::new();

    for report in reports {
        if !report_ids.contains(&report.id) {
            unique_reports.push(report.clone());
            report_ids.insert(report.id);
        }
    }

    Ok(unique_reports)
}

fn get_field<'a>(
    row: &'a Vec<AnyValue<'a>>,
    column_name: &'a str,
    column_names: &'a Vec<String>,
) -> Option<AnyValue<'a>> {
    match column_names.iter().position(|x| x == column_name) {
        Some(index) => row.get(index).cloned(),
        None => None,
    }
}
