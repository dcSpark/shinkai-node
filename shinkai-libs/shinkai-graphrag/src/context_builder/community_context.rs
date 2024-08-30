use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use polars::{
    frame::DataFrame,
    io::SerWriter,
    prelude::{col, concat, CsvWriter, DataType, IntoLazy, LazyFrame, NamedFrom, SortMultipleOptions, UnionArgs},
    series::Series,
};
use rand::prelude::SliceRandom;
use tiktoken_rs::tokenizer::Tokenizer;

use crate::llm::utils::num_tokens;

use super::{context_builder::ContextBuilderParams, indexer_entities::Entity, indexer_reports::CommunityReport};

pub struct GlobalCommunityContext {
    community_reports: Vec<CommunityReport>,
    entities: Option<Vec<Entity>>,
    token_encoder: Option<Tokenizer>,
}

impl GlobalCommunityContext {
    pub fn new(
        community_reports: Vec<CommunityReport>,
        entities: Option<Vec<Entity>>,
        token_encoder: Option<Tokenizer>,
    ) -> Self {
        Self {
            community_reports,
            entities,
            token_encoder,
        }
    }

    pub async fn build_context(
        &self,
        context_builder_params: ContextBuilderParams,
    ) -> anyhow::Result<(Vec<String>, HashMap<String, DataFrame>)> {
        let ContextBuilderParams {
            use_community_summary,
            column_delimiter,
            shuffle_data,
            include_community_rank,
            min_community_rank,
            community_rank_name,
            include_community_weight,
            community_weight_name,
            normalize_community_weight,
            max_tokens,
            context_name,
        } = context_builder_params;

        let (community_context, community_context_data) = CommunityContext::build_community_context(
            self.community_reports.clone(),
            self.entities.clone(),
            self.token_encoder.clone(),
            use_community_summary,
            &column_delimiter,
            shuffle_data,
            include_community_rank,
            min_community_rank,
            &community_rank_name,
            include_community_weight,
            &community_weight_name,
            normalize_community_weight,
            max_tokens,
            false,
            &context_name,
        )?;

        let final_context = community_context;
        let final_context_data = community_context_data;

        Ok((final_context, final_context_data))
    }
}

pub struct CommunityContext {}

impl CommunityContext {
    pub fn build_community_context(
        community_reports: Vec<CommunityReport>,
        entities: Option<Vec<Entity>>,
        token_encoder: Option<Tokenizer>,
        use_community_summary: bool,
        column_delimiter: &str,
        shuffle_data: bool,
        include_community_rank: bool,
        min_community_rank: u32,
        community_rank_name: &str,
        include_community_weight: bool,
        community_weight_name: &str,
        normalize_community_weight: bool,
        max_tokens: usize,
        single_batch: bool,
        context_name: &str,
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
            community_reports = Self::_compute_community_weights(
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

        let header = _get_header(attributes.clone());
        let mut all_context_text: Vec<String> = Vec::new();
        let mut all_context_records: Vec<DataFrame> = Vec::new();

        let mut batch = Batch::new();

        batch.init_batch(context_name, &header, column_delimiter, token_encoder);

        for report in selected_reports {
            let (new_context_text, new_context) = _report_context_text(&report, &attributes);
            let new_tokens = num_tokens(&new_context_text, token_encoder);

            // add the current batch to the context data and start a new batch if we are in multi-batch mode
            if batch.batch_tokens + new_tokens > max_tokens {
                batch.cut_batch(
                    &mut all_context_text,
                    &mut all_context_records,
                    entities.clone(),
                    &header,
                    community_weight_name,
                    community_rank_name,
                    include_community_weight,
                    include_community_rank,
                )?;

                if single_batch {
                    break;
                }

                batch.init_batch(context_name, &header, column_delimiter, token_encoder);
            }

            batch.batch_text.push_str(&new_context_text);
            batch.batch_tokens += new_tokens;
            batch.batch_records.push(new_context);
        }

        if !all_context_text.contains(&batch.batch_text) {
            batch.cut_batch(
                &mut all_context_text,
                &mut all_context_records,
                entities.clone(),
                &header,
                community_weight_name,
                community_rank_name,
                include_community_weight,
                include_community_rank,
            )?;
        }

        if all_context_records.is_empty() {
            eprintln!("Warning: No community records added when building community context.");
            return Ok((Vec::new(), HashMap::new()));
        }

        let records_concat = concat(
            all_context_records
                .into_iter()
                .map(|df| df.lazy())
                .collect::<Vec<LazyFrame>>(),
            UnionArgs::default(),
        )?
        .collect()?;

        Ok((
            all_context_text,
            HashMap::from([(context_name.to_lowercase(), records_concat)]),
        ))
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
}

struct Batch {
    batch_text: String,
    batch_tokens: usize,
    batch_records: Vec<Vec<String>>,
}

impl Batch {
    fn new() -> Self {
        Batch {
            batch_text: String::new(),
            batch_tokens: 0,
            batch_records: Vec::new(),
        }
    }

    fn init_batch(
        &mut self,
        context_name: &str,
        header: &Vec<String>,
        column_delimiter: &str,
        token_encoder: Option<Tokenizer>,
    ) {
        self.batch_text = format!("-----{}-----\n{}\n", context_name, header.join(column_delimiter));
        self.batch_tokens = num_tokens(&self.batch_text, token_encoder);
        self.batch_records.clear();
    }

    fn cut_batch(
        &mut self,
        all_context_text: &mut Vec<String>,
        all_context_records: &mut Vec<DataFrame>,
        entities: Option<Vec<Entity>>,
        header: &Vec<String>,
        community_weight_name: &str,
        community_rank_name: &str,
        include_community_weight: bool,
        include_community_rank: bool,
    ) -> anyhow::Result<()> {
        let weight_column = if include_community_weight && entities.is_some_and(|e| !e.is_empty()) {
            Some(community_weight_name)
        } else {
            None
        };
        let rank_column = if include_community_rank {
            Some(community_rank_name)
        } else {
            None
        };

        let mut record_df = Self::_convert_report_context_to_df(
            self.batch_records.clone(),
            header.clone(),
            weight_column,
            rank_column,
        )?;
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

        return Self::_rank_report_context(record_df, weight_column, rank_column);
    }

    fn _rank_report_context(
        report_df: DataFrame,
        weight_column: Option<&str>,
        rank_column: Option<&str>,
    ) -> anyhow::Result<DataFrame> {
        let mut rank_attributes = Vec::new();

        let mut report_df = report_df;

        if let Some(weight_column) = weight_column {
            rank_attributes.push(weight_column);
            report_df = report_df
                .clone()
                .lazy()
                .with_column(col(weight_column).cast(DataType::Float64))
                .collect()?;
        }

        if let Some(rank_column) = rank_column {
            rank_attributes.push(rank_column);
            report_df = report_df
                .clone()
                .lazy()
                .with_column(col(rank_column).cast(DataType::Float64))
                .collect()?;
        }

        if !rank_attributes.is_empty() {
            report_df = report_df
                .clone()
                .lazy()
                .sort(rank_attributes, SortMultipleOptions::new().with_order_descending(true))
                .collect()?;
        }

        Ok(report_df)
    }
}
