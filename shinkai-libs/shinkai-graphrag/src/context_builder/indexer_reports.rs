use std::collections::HashMap;

use polars::prelude::*;
use polars_lazy::dsl::col;
use serde::{Deserialize, Serialize};

pub fn read_indexer_reports(
    final_community_reports: &DataFrame,
    final_nodes: &DataFrame,
    community_level: u32,
) -> anyhow::Result<Vec<CommunityReport>> {
    let entity_df = final_nodes.clone();
    let entity_df = filter_under_community_level(&entity_df, community_level)?;

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
        .with_column(col("community").cast(DataType::String))
        .collect()?;

    let entity_df = entity_df
        .clone()
        .lazy()
        .group_by([col("title")])
        .agg([col("community").max()])
        .collect()?;

    let filtered_community_df = entity_df
        .clone()
        .lazy()
        .filter(len().over([col("community")]).gt(lit(1)))
        .collect()?;

    let report_df = final_community_reports.clone();
    let report_df = filter_under_community_level(&report_df, community_level)?;

    let report_df = report_df
        .clone()
        .lazy()
        .join(
            filtered_community_df.clone().lazy(),
            [col("community")],
            [col("community")],
            JoinArgs::new(JoinType::Inner),
        )
        .collect()?;

    let reports = read_community_reports(&report_df, "community", Some("community"), None, None)?;
    Ok(reports)
}

pub fn filter_under_community_level(df: &DataFrame, community_level: u32) -> anyhow::Result<DataFrame> {
    let mask = df.column("level")?.i64()?.lt_eq(community_level);
    let result = df.filter(&mask)?;

    Ok(result)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommunityReport {
    pub id: String,
    pub short_id: Option<String>,
    pub title: String,
    pub community_id: String,
    pub summary: String,
    pub full_content: String,
    pub rank: Option<f64>,
    pub summary_embedding: Option<Vec<f64>>,
    pub full_content_embedding: Option<Vec<f64>>,
    pub attributes: Option<HashMap<String, String>>,
}

pub fn read_community_reports(
    df: &DataFrame,
    _id_col: &str,
    _short_id_col: Option<&str>,
    // title_col: &str,
    // community_col: &str,
    // summary_col: &str,
    // content_col: &str,
    // rank_col: Option<&str>,
    _summary_embedding_col: Option<&str>,
    _content_embedding_col: Option<&str>,
    // attributes_cols: Option<&[&str]>,
) -> anyhow::Result<Vec<CommunityReport>> {
    let mut df = df.clone();
    df.as_single_chunk_par();
    let mut iters = df
        .columns(["community", "title", "summary", "full_content", "rank"])?
        .iter()
        .map(|s| s.iter())
        .collect::<Vec<_>>();

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

    let mut reports = Vec::new();
    for row in rows {
        let report = CommunityReport {
            id: row.get(0).unwrap_or(&String::new()).to_string(),
            short_id: Some(row.get(0).unwrap_or(&String::new()).to_string()),
            title: row.get(1).unwrap_or(&String::new()).to_string(),
            community_id: row.get(0).unwrap_or(&String::new()).to_string(),
            summary: row.get(2).unwrap_or(&String::new()).to_string(),
            full_content: row.get(3).unwrap_or(&String::new()).to_string(),
            rank: Some(row.get(4).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)),
            summary_embedding: None,
            full_content_embedding: None,
            attributes: None,
        };
        reports.push(report);
    }

    Ok(reports)
}
