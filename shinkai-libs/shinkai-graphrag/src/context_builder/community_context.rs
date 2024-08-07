use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use polars::{
    frame::DataFrame,
    io::SerWriter,
    prelude::{col, CsvWriter, DataType, IntoLazy, NamedFrom, SortMultipleOptions},
    series::Series,
};
use rand::prelude::SliceRandom;
use tiktoken_rs::tokenizer::Tokenizer;

use crate::llm::utils::num_tokens;

use super::{context_builder::ConversationHistory, indexer_entities::Entity, indexer_reports::CommunityReport};

pub struct GlobalCommunityContext {
    community_reports: Vec<CommunityReport>,
    entities: Option<Vec<Entity>>,
    token_encoder: Option<Tokenizer>,
    random_state: i32,
}

impl GlobalCommunityContext {
    pub fn new(
        community_reports: Vec<CommunityReport>,
        entities: Option<Vec<Entity>>,
        token_encoder: Option<Tokenizer>,
        random_state: Option<i32>,
    ) -> Self {
        Self {
            community_reports,
            entities,
            token_encoder,
            random_state: random_state.unwrap_or(86),
        }
    }

    pub async fn build_context(
        &self,
        conversation_history: Option<ConversationHistory>,
        context_builder_params: Option<HashMap<String, serde_json::Value>>,
    ) -> (Vec<String>, HashMap<String, DataFrame>) {
        (vec![], HashMap::new())
    }
}

pub fn build_community_context(
    community_reports: Vec<CommunityReport>,
    entities: Option<Vec<Entity>>,
    token_encoder: Option<Tokenizer>,
    use_community_summary: bool,
    column_delimiter: &str,
    shuffle_data: bool,
    include_community_rank: bool,
    min_community_rank: i32,
    community_rank_name: &str,
    include_community_weight: bool,
    community_weight_name: &str,
    normalize_community_weight: bool,
    max_tokens: i32,
    single_batch: bool,
    context_name: &str,
    random_state: i32,
) -> anyhow::Result<(Vec<String>, HashMap<String, DataFrame>)> {
    let _is_included = |report: &CommunityReport| -> bool {
        report.rank.is_some() && report.rank.unwrap() >= min_community_rank.into()
    };

    let _get_header = |attributes: Vec<String>| -> Vec<String> {
        let mut header = vec!["id".to_string(), "title".to_string()];
        let mut filtered_attributes: Vec<String> = attributes
            .iter()
            .filter(|&col| !header.contains(&col.to_string()))
            .cloned()
            .collect();

        if !include_community_weight {
            filtered_attributes.retain(|col| col != community_weight_name);
        }

        header.extend(filtered_attributes.into_iter().map(|s| s.to_string()));
        header.push(if use_community_summary {
            "summary".to_string()
        } else {
            "content".to_string()
        });

        if include_community_rank {
            header.push(community_rank_name.to_string());
        }

        header
    };

    let _report_context_text = |report: &CommunityReport, attributes: &[String]| -> (String, Vec<String>) {
        let mut context: Vec<String> = vec![report.short_id.clone().unwrap_or_default(), report.title.clone()];

        for field in attributes {
            let value = report
                .attributes
                .as_ref()
                .and_then(|attrs| attrs.get(field))
                .cloned()
                .unwrap_or_default();
            context.push(value);
        }

        context.push(if use_community_summary {
            report.summary.clone()
        } else {
            report.full_content.clone()
        });

        if include_community_rank {
            context.push(report.rank.unwrap_or_default().to_string());
        }

        let result = context.join(column_delimiter) + "\n";
        (result, context)
    };

    let compute_community_weights = entities.is_some()
        && !community_reports.is_empty()
        && include_community_weight
        && (community_reports[0].attributes.is_none()
            || !community_reports[0]
                .attributes
                .clone()
                .unwrap()
                .contains_key(community_weight_name));

    let mut community_reports = community_reports;
    if compute_community_weights {
        community_reports = _compute_community_weights(
            community_reports,
            entities.clone(),
            community_weight_name,
            normalize_community_weight,
        );
    }

    let mut selected_reports: Vec<CommunityReport> = community_reports
        .iter()
        .filter(|&report| _is_included(report))
        .cloned()
        .collect();

    if selected_reports.is_empty() {
        return Ok((Vec::new(), HashMap::new()));
    }

    if shuffle_data {
        let mut rng = rand::thread_rng();
        selected_reports.shuffle(&mut rng);
    }

    let attributes = if let Some(attributes) = &community_reports[0].attributes {
        attributes.keys().cloned().collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let header = _get_header(attributes);
    let mut all_context_text: Vec<String> = Vec::new();
    let mut all_context_records: Vec<DataFrame> = Vec::new();

    let mut batch_text = String::new();
    let mut batch_tokens = 0;
    let mut batch_records: Vec<Vec<String>> = Vec::new();

    let mut _init_batch = || {
        batch_text = format!("-----{}-----\n{}\n", context_name, header.join(column_delimiter));
        batch_tokens = num_tokens(&batch_text, token_encoder);
        batch_records = Vec::new();
    };

    let _cut_batch = |batch_records: Vec<Vec<String>>, header: Vec<String>| -> anyhow::Result<()> {
        let weight_column = if include_community_weight && entities.is_some() {
            Some(community_weight_name)
        } else {
            None
        };
        let rank_column = if include_community_rank {
            Some(community_rank_name)
        } else {
            None
        };

        let mut record_df = _convert_report_context_to_df(batch_records, header, weight_column, rank_column)?;
        if record_df.is_empty() {
            return Ok(());
        }

        let mut buffer = Cursor::new(Vec::new());
        CsvWriter::new(buffer.clone()).finish(&mut record_df).unwrap();

        let mut current_context_text = String::new();
        buffer.read_to_string(&mut current_context_text)?;

        all_context_text.push(current_context_text);
        all_context_records.push(record_df);

        Ok(())
    };

    _init_batch();

    Ok((vec![], HashMap::new()))
}

fn _compute_community_weights(
    community_reports: Vec<CommunityReport>,
    entities: Option<Vec<Entity>>,
    weight_attribute: &str,
    normalize: bool,
) -> Vec<CommunityReport> {
    // Calculate a community's weight as the count of text units associated with entities within the community.
    if let Some(entities) = entities {
        let mut community_reports = community_reports.clone();
        let mut community_text_units = std::collections::HashMap::new();
        for entity in entities {
            if let Some(community_ids) = entity.community_ids.clone() {
                for community_id in community_ids {
                    community_text_units
                        .entry(community_id)
                        .or_insert_with(Vec::new)
                        .extend(entity.text_unit_ids.clone());
                }
            }
        }
        for report in &mut community_reports {
            if report.attributes.is_none() {
                report.attributes = Some(std::collections::HashMap::new());
            }
            if let Some(attributes) = &mut report.attributes {
                attributes.insert(
                    weight_attribute.to_string(),
                    community_text_units
                        .get(&report.community_id)
                        .map(|text_units| text_units.len())
                        .unwrap_or(0)
                        .to_string(),
                );
            }
        }
        if normalize {
            // Normalize by max weight
            let all_weights: Vec<f64> = community_reports
                .iter()
                .filter_map(|report| {
                    report
                        .attributes
                        .as_ref()
                        .and_then(|attributes| attributes.get(weight_attribute))
                        .map(|weight| weight.parse::<f64>().unwrap_or(0.0))
                })
                .collect();
            if let Some(max_weight) = all_weights.iter().cloned().max_by(|a, b| a.partial_cmp(b).unwrap()) {
                for mut report in community_reports {
                    if let Some(attributes) = &mut report.attributes {
                        if let Some(weight) = attributes.get_mut(weight_attribute) {
                            *weight = (weight.parse::<f64>().unwrap_or(0.0) / max_weight).to_string();
                        }
                    }
                }
            }
        }
    }
    community_reports
}

fn _convert_report_context_to_df(
    context_records: Vec<Vec<String>>,
    header: Vec<String>,
    weight_column: Option<&str>,
    rank_column: Option<&str>,
) -> anyhow::Result<DataFrame> {
    if context_records.is_empty() {
        return Ok(DataFrame::empty());
    }

    let mut data_series = Vec::new();
    for (header, records) in header.iter().zip(context_records.iter()) {
        let series = Series::new(header, records);
        data_series.push(series);
    }

    let record_df = DataFrame::new(data_series)?;

    return _rank_report_context(record_df, weight_column, rank_column);
}

fn _rank_report_context(
    report_df: DataFrame,
    weight_column: Option<&str>,
    rank_column: Option<&str>,
) -> anyhow::Result<DataFrame> {
    let weight_column = weight_column.unwrap_or("occurrence weight");
    let rank_column = rank_column.unwrap_or("rank");

    let mut rank_attributes = Vec::new();
    rank_attributes.push(weight_column);
    let report_df = report_df
        .clone()
        .lazy()
        .with_column(col(weight_column).cast(DataType::Float64))
        .collect()?;

    rank_attributes.push(rank_column);
    let report_df = report_df
        .clone()
        .lazy()
        .with_column(col(rank_column).cast(DataType::Float64))
        .collect()?;

    let report_df = report_df
        .clone()
        .lazy()
        .sort(rank_attributes, SortMultipleOptions::new().with_order_descending(true))
        .collect()?;

    Ok(report_df)
}
