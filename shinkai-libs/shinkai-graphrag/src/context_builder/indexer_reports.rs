use std::collections::HashSet;

use polars::prelude::*;
use polars_lazy::dsl::col;

use crate::models::CommunityReport;

use super::indexer_entities::get_field;

pub fn read_indexer_reports(
    final_community_reports: &DataFrame,
    final_nodes: &DataFrame,
    community_level: u32,
) -> anyhow::Result<Vec<CommunityReport>> {
    let entity_df = final_nodes.clone();
    let entity_df = filter_under_community_level(&entity_df, community_level)?;

    let filtered_community_df = entity_df
        .lazy()
        .with_column(col("community").fill_null(lit(-1)))
        .with_column(col("community").cast(DataType::Int32))
        .group_by([col("title")])
        .agg([col("community").max()])
        .with_column(col("community").cast(DataType::String))
        .filter(len().over([col("community")]).gt(lit(1)))
        .collect()?;

    let report_df = final_community_reports.clone();
    let report_df = filter_under_community_level(&report_df, community_level)?;

    let report_df = report_df
        .lazy()
        .join(
            filtered_community_df.lazy(),
            [col("community")],
            [col("community")],
            JoinArgs::new(JoinType::Inner),
        )
        .collect()?;

    let reports = read_community_reports(
        report_df,
        "community",
        Some("community"),
        "title",
        "community",
        "summary",
        "full_content",
        Some("rank"),
        None,
        None,
    )?;
    Ok(reports)
}

pub fn filter_under_community_level(df: &DataFrame, community_level: u32) -> anyhow::Result<DataFrame> {
    let mask = df.column("level")?.i64()?.lt_eq(community_level);
    let result = df.filter(&mask)?;

    Ok(result)
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
