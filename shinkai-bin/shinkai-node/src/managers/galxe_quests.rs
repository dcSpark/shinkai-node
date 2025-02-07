use blake3;
use chrono::{DateTime, Utc};
use hex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestStatus {
    NotStarted,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestProgress {
    pub status: QuestStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_count: u32,
    pub required_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum QuestType {
    CreateIdentity,
    DownloadFromStore,
    ReturnAfterDays,
    CreateTool,
    SubmitAndGetApproval,
    TopRanking,
    WriteFeedback,
    WriteHonestReview,
    UseRAG,
    UseSpotlight,
    InstallCommunityTools,
    WriteAppReviews,
}

pub async fn compute_quests(
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
) -> Result<HashMap<QuestType, bool>, String> {
    let mut quests: HashMap<QuestType, bool> = HashMap::new();

    // Create Identity Quest
    quests.insert(
        QuestType::CreateIdentity,
        compute_create_identity_quest(db.clone(), node_name.clone()).await?,
    );

    // // Download from Store Quest
    // quests.insert(
    //     QuestType::DownloadFromStore,
    //     compute_download_store_quest(db.clone(), now)?,
    // );

    // // Return After Days Quest
    // quests.insert(
    //     QuestType::ReturnAfterDays,
    //     compute_return_after_days_quest(db.clone(), now)?,
    // );

    // // Create Tool Quest
    // quests.insert(QuestType::CreateTool, compute_create_tool_quest(db.clone(), now)?);

    // // Submit and Get Approval Quest
    // quests.insert(
    //     QuestType::SubmitAndGetApproval,
    //     compute_submit_approval_quest(db.clone(), now)?,
    // );

    // // Top Ranking Quest
    // quests.insert(QuestType::TopRanking, compute_top_ranking_quest(db.clone(), now)?);

    // // Write Feedback Quest
    // quests.insert(QuestType::WriteFeedback, compute_write_feedback_quest(db.clone(), now)?);

    // // Write Honest Review Quest
    // quests.insert(
    //     QuestType::WriteHonestReview,
    //     compute_write_review_quest(db.clone(), now)?,
    // );

    // // Use RAG Quest
    // quests.insert(QuestType::UseRAG, compute_use_rag_quest(db.clone(), now)?);

    // // Use Spotlight Quest
    // quests.insert(QuestType::UseSpotlight, compute_use_spotlight_quest(db.clone(), now)?);

    // // Install Community Tools Quest
    // quests.insert(
    //     QuestType::InstallCommunityTools,
    //     compute_install_community_tools_quest(db.clone(), now)?,
    // );

    // // Write App Reviews Quest
    // quests.insert(
    //     QuestType::WriteAppReviews,
    //     compute_write_app_reviews_quest(db.clone(), now)?,
    // );

    Ok(quests)
}

pub async fn compute_create_identity_quest(db: Arc<SqliteManager>, node_name: ShinkaiName) -> Result<bool, String> {
    // First check if the node name is localhost
    if node_name.to_string() == "@@localhost.sep-shinkai" {
        println!("Identity is localhost, quest not completed");
        return Ok(false);
    }

    let node_info = db.query_row(
        "SELECT node_name, node_encryption_public_key, node_signature_public_key FROM local_node_keys LIMIT 1",
        params![],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        },
    );

    let has_identity = match node_info {
        Ok((name, local_enc_key, local_sig_key)) => {
            // Get registry data
            let registry = ShinkaiRegistry::new(
                "https://sepolia.base.org",
                "0x425fb20ba3874e887336aaa7f3fab32d08135ba9",
                None,
            )
            .await
            .map_err(|e| format!("Failed to create registry: {}", e))?;

            let onchain_identity = match registry.get_identity_record(node_name.to_string()).await {
                Ok(identity) => identity,
                Err(e) => {
                    println!("Identity not found in registry: {}", e);
                    return Ok(false);
                }
            };

            // Hash our local keys
            let local_enc_hash = blake3::hash(&local_enc_key).to_hex().to_string();
            let local_sig_hash = blake3::hash(&local_sig_key).to_hex().to_string();

            // The registry keys are already hex strings of the hashes, no need to decode and hash again
            let registry_enc_hash = onchain_identity.encryption_key;
            let registry_sig_hash = onchain_identity.signature_key;

            println!(
                "Node Identity Found:\nName: {}\nLocal Encryption Key Hash: {}\nLocal Signature Key Hash: {}\nRegistry Encryption Key Hash: {}\nRegistry Signature Key Hash: {}",
                name,
                local_enc_hash,
                local_sig_hash,
                registry_enc_hash,
                registry_sig_hash
            );

            // Compare the hex strings directly
            let keys_match = local_enc_hash == registry_enc_hash && local_sig_hash == registry_sig_hash;
            println!("Keys match with registry: {}", keys_match);
            keys_match
        }
        Err(_) => false,
    };

    println!("Has valid identity: {}", has_identity);
    Ok(has_identity)
}

fn compute_download_store_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let has_downloaded = db
        .query_row("SELECT COUNT(*) FROM downloads", params![], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("Failed to check downloads: {}", e))?
        > 0;

    Ok(create_progress(
        if has_downloaded { 1 } else { 0 },
        1,
        if has_downloaded { Some(now) } else { None },
    ))
}

fn compute_return_after_days_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let first_activity_date: Option<String> = db
        .query_row("SELECT MIN(created_at) FROM user_activities", params![], |row| {
            row.get(0)
        })
        .ok();

    let first_activity_date = first_activity_date.and_then(|date_str| {
        DateTime::parse_from_rfc3339(&date_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    if let Some(first_date) = first_activity_date {
        let days = (now - first_date).num_days() as u32;
        Ok(create_progress(days, 7, Some(first_date)))
    } else {
        Ok(create_progress(0, 7, None))
    }
}

fn compute_create_tool_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let tools_created = db
        .query_row("SELECT COUNT(*) FROM tools", params![], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("Failed to count tools: {}", e))? as u32;

    Ok(create_progress(
        tools_created,
        1,
        if tools_created > 0 { Some(now) } else { None },
    ))
}

fn compute_submit_approval_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let tools_approved = db
        .query_row(
            "SELECT COUNT(*) FROM tools WHERE status = 'approved'",
            params![],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Failed to count approved tools: {}", e))? as u32;

    Ok(create_progress(
        tools_approved,
        1,
        if tools_approved > 0 { Some(now) } else { None },
    ))
}

fn compute_top_ranking_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let current_rank = db
        .query_row("SELECT MIN(ranking) FROM user_rankings", params![], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(1000) as u32;

    Ok(create_progress(
        if current_rank <= 50 { 1 } else { 0 },
        1,
        if current_rank <= 50 { Some(now) } else { None },
    ))
}

fn compute_write_feedback_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let feedback_count = db
        .query_row("SELECT COUNT(*) FROM feedback", params![], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("Failed to count feedback: {}", e))? as u32;

    Ok(create_progress(
        feedback_count,
        1,
        if feedback_count > 0 { Some(now) } else { None },
    ))
}

fn compute_write_review_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let review_count = db
        .query_row("SELECT COUNT(*) FROM reviews", params![], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("Failed to count reviews: {}", e))? as u32;

    Ok(create_progress(
        review_count,
        1,
        if review_count > 0 { Some(now) } else { None },
    ))
}

fn compute_use_rag_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let days_used_rag = db
        .query_row(
            "SELECT COUNT(DISTINCT DATE(created_at)) FROM rag_usage",
            params![],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Failed to count RAG usage days: {}", e))? as u32;

    Ok(create_progress(
        days_used_rag,
        3,
        if days_used_rag > 0 { Some(now) } else { None },
    ))
}

fn compute_use_spotlight_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let days_used_spotlight = db
        .query_row(
            "SELECT COUNT(DISTINCT DATE(created_at)) FROM spotlight_usage",
            params![],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Failed to count Spotlight usage days: {}", e))? as u32;

    Ok(create_progress(
        days_used_spotlight,
        3,
        if days_used_spotlight > 0 { Some(now) } else { None },
    ))
}

fn compute_install_community_tools_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let installed_tools = db
        .query_row("SELECT COUNT(*) FROM installed_tools", params![], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|e| format!("Failed to count installed tools: {}", e))? as u32;

    Ok(create_progress(
        installed_tools,
        3,
        if installed_tools > 0 { Some(now) } else { None },
    ))
}

fn compute_write_app_reviews_quest(db: Arc<SqliteManager>, now: DateTime<Utc>) -> Result<QuestProgress, String> {
    let app_reviews = db
        .query_row("SELECT COUNT(*) FROM app_reviews", params![], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|e| format!("Failed to count app reviews: {}", e))? as u32;

    Ok(create_progress(
        app_reviews,
        3,
        if app_reviews > 0 { Some(now) } else { None },
    ))
}

fn create_progress(current: u32, required: u32, started: Option<DateTime<Utc>>) -> QuestProgress {
    let now = Utc::now();
    let completed = current >= required;
    QuestProgress {
        status: if completed {
            QuestStatus::Completed
        } else if started.is_some() {
            QuestStatus::InProgress
        } else {
            QuestStatus::NotStarted
        },
        started_at: started,
        completed_at: if completed { Some(now) } else { None },
        current_count: current,
        required_count: required,
    }
}
