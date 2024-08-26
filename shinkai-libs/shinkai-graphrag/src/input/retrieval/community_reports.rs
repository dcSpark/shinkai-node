use std::collections::{HashMap, HashSet};

use polars::{frame::DataFrame, prelude::NamedFrom, series::Series};

use crate::models::{CommunityReport, Entity};

pub fn get_candidate_communities(
    selected_entities: Vec<Entity>,
    community_reports: Vec<CommunityReport>,
    include_community_rank: bool,
    use_community_summary: bool,
) -> anyhow::Result<DataFrame> {
    let mut selected_community_ids: HashSet<String> = HashSet::new();
    for entity in &selected_entities {
        if let Some(community_ids) = &entity.community_ids {
            selected_community_ids.extend(community_ids.iter().cloned());
        }
    }

    let mut selected_reports: Vec<CommunityReport> = Vec::new();
    for community in &community_reports {
        if selected_community_ids.contains(&community.id) {
            selected_reports.push(community.clone());
        }
    }

    to_community_report_dataframe(selected_reports, include_community_rank, use_community_summary)
}

pub fn to_community_report_dataframe(
    reports: Vec<CommunityReport>,
    include_community_rank: bool,
    use_community_summary: bool,
) -> anyhow::Result<DataFrame> {
    if reports.is_empty() {
        return Ok(DataFrame::default());
    }

    let mut header = vec!["id".to_string(), "title".to_string()];
    let attribute_cols: Vec<String> = reports[0]
        .attributes
        .as_ref()
        .map(|attrs| attrs.keys().filter(|&col| !header.contains(&col)).cloned().collect())
        .unwrap_or_default();

    header.extend(attribute_cols.iter().cloned());
    header.push(if use_community_summary { "summary" } else { "content" }.to_string());
    if include_community_rank {
        header.push("rank".to_string());
    }

    let mut records = HashMap::new();
    for report in reports {
        records
            .entry("id")
            .or_insert_with(Vec::new)
            .push(report.short_id.unwrap_or_default());
        records.entry("title").or_insert_with(Vec::new).push(report.title);

        for field in &attribute_cols {
            records.entry(field).or_insert_with(Vec::new).push(
                report
                    .attributes
                    .as_ref()
                    .and_then(|attrs| attrs.get(field))
                    .cloned()
                    .unwrap_or_default(),
            );
        }

        if use_community_summary {
            records.entry("summary").or_insert_with(Vec::new).push(report.summary);
        } else {
            records
                .entry("content")
                .or_insert_with(Vec::new)
                .push(report.full_content);
        }

        if include_community_rank {
            records
                .entry("rank")
                .or_insert_with(Vec::new)
                .push(report.rank.map(|r| r.to_string()).unwrap_or_default());
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

    let record_df = DataFrame::new(data_series)?;

    Ok(record_df)
}
