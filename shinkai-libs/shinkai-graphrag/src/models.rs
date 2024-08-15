use std::collections::HashMap;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: String,
    pub short_id: Option<String>,
    pub source: String,
    pub target: String,
    pub weight: Option<f64>,
    pub description: Option<String>,
    pub description_embedding: Option<Vec<f64>>,
    pub text_unit_ids: Option<Vec<String>>,
    pub document_ids: Option<Vec<String>>,
    pub attributes: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct TextUnit {
    pub id: String,
    pub short_id: Option<String>,
    pub text: String,
    pub text_embedding: Option<Vec<f64>>,
    pub entity_ids: Option<Vec<String>>,
    pub relationship_ids: Option<Vec<String>>,
    pub n_tokens: Option<i32>,
    pub document_ids: Option<Vec<String>>,
    pub attributes: Option<HashMap<String, String>>,
}
