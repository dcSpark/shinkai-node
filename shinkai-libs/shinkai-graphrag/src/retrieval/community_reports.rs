use std::collections::HashSet;

use polars::frame::DataFrame;

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

    Ok(DataFrame::default())
}
