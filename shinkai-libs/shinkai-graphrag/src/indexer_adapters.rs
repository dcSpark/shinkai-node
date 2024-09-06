use std::vec;

use polars::prelude::*;
use polars_lazy::dsl::col;

use crate::{
    input::loaders::dfs::{read_community_reports, read_entities, read_relationships, read_text_units},
    models::{CommunityReport, Entity, Relationship, TextUnit},
};

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

pub fn read_indexer_relationships(final_relationships: &DataFrame) -> anyhow::Result<Vec<Relationship>> {
    let relationships = read_relationships(
        final_relationships.clone(),
        "id",
        Some("human_readable_id"),
        "source",
        "target",
        Some("description"),
        None,
        Some("weight"),
        Some("text_unit_ids"),
        None,
        Some(vec!["rank"]),
    )?;

    Ok(relationships)
}

pub fn read_indexer_text_units(final_text_units: &DataFrame) -> anyhow::Result<Vec<TextUnit>> {
    let text_units = read_text_units(
        final_text_units.clone(),
        "id",
        None,
        "text",
        Some("entity_ids"),
        Some("relationship_ids"),
        Some("n_tokens"),
        Some("document_ids"),
        Some("text_embedding"),
        None,
    )?;

    Ok(text_units)
}

fn filter_under_community_level(df: &DataFrame, community_level: u32) -> anyhow::Result<DataFrame> {
    let mask = df.column("level")?.i64()?.lt_eq(community_level);
    let result = df.filter(&mask)?;

    Ok(result)
}
