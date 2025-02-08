use blake3;
use chrono::{DateTime, Utc};
use reqwest;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json;
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use std::collections::HashMap;
use std::sync::Arc;

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
    CreateIdentity,       // Done
    DownloadFromStore,    // Done
    ReturnFor7Days,       // Done
    ReturnFor3Days,       // Done
    ReturnFor2Days,       // Done
    CreateTool,           // Done
    SubmitAndGetApproval, // Done
    TopRanking,           // Done
    WriteFeedback,        // Done
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

    // Download from Store Quest
    quests.insert(
        QuestType::DownloadFromStore,
        compute_download_store_quest(db.clone()).await?,
    );

    // Return For Days Quests
    quests.insert(
        QuestType::ReturnFor7Days,
        compute_return_for_days_quest(db.clone(), 7).await?,
    );
    quests.insert(
        QuestType::ReturnFor3Days,
        compute_return_for_days_quest(db.clone(), 3).await?,
    );
    quests.insert(
        QuestType::ReturnFor2Days,
        compute_return_for_days_quest(db.clone(), 2).await?,
    );

    // Create Tool Quest
    quests.insert(QuestType::CreateTool, compute_create_tool_quest(db.clone()).await?);

    // Submit and Get Approval Quest
    quests.insert(
        QuestType::SubmitAndGetApproval,
        compute_submit_approval_quest(db.clone(), node_name.clone()).await?,
    );

    // Top Ranking Quest
    quests.insert(
        QuestType::TopRanking,
        compute_top_ranking_quest(db.clone(), node_name.clone()).await?,
    );

    // Write Feedback Quest
    quests.insert(
        QuestType::WriteFeedback,
        compute_write_feedback_quest(db.clone(), node_name).await?,
    );

    // // Write Honest Review Quest
    // quests.insert(
    //     QuestType::WriteHonestReview,
    //     compute_write_review_quest(db.clone(), now)?,
    // );

    // // Use RAG Quest
    quests.insert(QuestType::UseRAG, compute_use_rag_quest(db.clone()).await?);

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
            keys_match
        }
        Err(_) => false,
    };

    println!("Has valid identity: {}", has_identity);
    Ok(has_identity)
}

pub async fn compute_download_store_quest(db: Arc<SqliteManager>) -> Result<bool, String> {
    // Get the list of default tools from the store
    let url = std::env::var("SHINKAI_TOOLS_DIRECTORY_URL")
        .unwrap_or_else(|_| "https://shinkai-store-302883622007.us-central1.run.app/store/defaults".to_string());

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("X-Shinkai-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch default tools: {}", e))?;

    if response.status() != 200 {
        return Err(format!(
            "Default tools request returned a non OK status: {}",
            response.status()
        ));
    }

    let default_tools: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse default tools: {}", e))?;

    // Get the router keys of default tools
    let default_tool_keys: std::collections::HashSet<String> = default_tools
        .iter()
        .filter_map(|tool| tool["routerKey"].as_str().map(|key| key.to_string()))
        .collect();

    println!("Default tool keys: {:?}", default_tool_keys);

    // Get all installed tools
    let installed_tools = db
        .get_all_tool_headers()
        .map_err(|e| format!("Failed to get installed tools: {}", e))?;

    // Count tools that were downloaded (exist in default tools but don't have playground)
    let downloaded_tools = installed_tools
        .iter()
        .filter(|tool| {
            let router_key = tool.tool_router_key.clone();
            let is_default = default_tool_keys.contains(&router_key);
            let is_deno_or_python = tool.tool_type == "Deno" || tool.tool_type == "Python";

            if !is_default {
                println!(
                    "Tool not in default list: {} (router key: {}, is_deno_or_python: {})",
                    tool.name, router_key, is_deno_or_python
                );
            }

            !is_default && is_deno_or_python
        })
        .count();

    println!("\nNumber of non-default Deno/Python tools: {}", downloaded_tools);
    Ok(downloaded_tools > 0)
}

pub async fn compute_return_for_days_quest(db: Arc<SqliteManager>, required_days: u32) -> Result<bool, String> {
    // Get all jobs
    let all_jobs = db.get_all_jobs().map_err(|e| format!("Failed to get jobs: {}", e))?;

    // Define the valid date range (Feb 9th to Feb 20th)
    let start_date = chrono::DateTime::parse_from_rfc3339("2025-02-08T00:00:00Z")
        .map_err(|e| format!("Failed to parse start date: {}", e))?
        .with_timezone(&chrono::Utc);
    let end_date = chrono::DateTime::parse_from_rfc3339("2025-02-20T23:59:59Z")
        .map_err(|e| format!("Failed to parse end date: {}", e))?
        .with_timezone(&chrono::Utc);

    // Collect unique dates when jobs were created
    let mut unique_dates = std::collections::HashSet::new();

    for job in all_jobs {
        // Parse the job's creation date
        let job_date = chrono::DateTime::parse_from_rfc3339(&job.datetime_created())
            .map_err(|e| format!("Failed to parse job date: {}", e))?
            .with_timezone(&chrono::Utc);

        // Check if the job was created within the valid date range
        if job_date >= start_date && job_date <= end_date {
            // Add the date (without time) to the set
            unique_dates.insert(job_date.date_naive());
        }
    }

    // Check if we have enough unique dates
    Ok(unique_dates.len() >= required_days as usize)
}

pub async fn compute_create_tool_quest(db: Arc<SqliteManager>) -> Result<bool, String> {
    // Get all playground tools
    let playground_tools = db
        .get_all_tool_playground()
        .map_err(|e| format!("Failed to get playground tools: {}", e))?;

    // Count tools that were created in the playground (not downloaded from store)
    let created_tools = playground_tools.len();

    println!("\nNumber of tools created in playground: {}", created_tools);
    Ok(created_tools > 0)
}

pub async fn compute_submit_approval_quest(db: Arc<SqliteManager>, node_name: ShinkaiName) -> Result<bool, String> {
    let url = format!("https://store-api.shinkai.com/user/{}/apps", node_name.to_string());

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("X-Shinkai-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user apps: {}", e))?;

    if response.status() != 200 {
        return Ok(false);
    }

    let apps: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse apps response: {}", e))?;

    // If we have any apps in the store, it means they were approved
    Ok(!apps.is_empty())
}

pub async fn compute_top_ranking_quest(db: Arc<SqliteManager>, node_name: ShinkaiName) -> Result<bool, String> {
    let url = format!("https://store-api.shinkai.com/user/{}/apps", node_name.to_string());

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("X-Shinkai-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user apps: {}", e))?;

    if response.status() != 200 {
        return Ok(false);
    }

    let apps: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse apps response: {}", e))?;

    // Check if any app is featured
    let has_featured = apps
        .iter()
        .any(|app| app.get("featured").and_then(|v| v.as_bool()).unwrap_or(false));

    Ok(has_featured)
}

pub async fn compute_write_feedback_quest(_db: Arc<SqliteManager>, node_name: ShinkaiName) -> Result<bool, String> {
    let url = format!("https://store-api.shinkai.com/user/{}/reviews", node_name.to_string());

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("X-Shinkai-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user reviews: {}", e))?;

    if response.status() != 200 {
        return Ok(false);
    }

    let reviews: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse reviews response: {}", e))?;

    // If we have any reviews, the quest is completed
    Ok(!reviews.is_empty())
}

pub async fn compute_use_rag_quest(db: Arc<SqliteManager>) -> Result<bool, String> {
    // Get all jobs
    let all_jobs = db.get_all_jobs().map_err(|e| format!("Failed to get jobs: {}", e))?;

    // Define the valid date range (Feb 9th to Feb 20th)
    let start_date = chrono::DateTime::parse_from_rfc3339("2025-02-08T00:00:00Z")
        .map_err(|e| format!("Failed to parse start date: {}", e))?
        .with_timezone(&chrono::Utc);
    let end_date = chrono::DateTime::parse_from_rfc3339("2025-02-20T23:59:59Z")
        .map_err(|e| format!("Failed to parse end date: {}", e))?
        .with_timezone(&chrono::Utc);

    // Collect unique dates when jobs with file resources were created
    let mut unique_dates = std::collections::HashSet::new();

    for job in all_jobs {
        // Parse the job's creation date
        let job_date = chrono::DateTime::parse_from_rfc3339(&job.datetime_created())
            .map_err(|e| format!("Failed to parse job date: {}", e))?
            .with_timezone(&chrono::Utc);

        // Check if the job was created within the valid date range
        if job_date >= start_date && job_date <= end_date {
            // Check if the job's scope contains any files or folders
            let scope = job.scope();
            if !scope.vector_fs_items.is_empty() || !scope.vector_fs_folders.is_empty() {
                // Add the date (without time) to the set
                unique_dates.insert(job_date.date_naive());
            }
        }
    }

    // Check if we have at least one day with file resource usage
    Ok(!unique_dates.is_empty())
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
