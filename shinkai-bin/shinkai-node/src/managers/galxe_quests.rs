use blake3::{self, Hasher};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, VerifyingKey};
use reqwest;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json;
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use x25519_dalek::PublicKey as EncryptionPublicKey;

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
    InstalledApp,
    CreateIdentity,
    DownloadFromStore,
    ComeBack2Days,
    ComeBack4Days,
    ComeBack7Days,
    CreateTool,
    SubmitAndGetApprovalForTool,
    SubmitAndGetApprovalFor2Tool,
    SubmitAndGetApprovalFor3Tool,
    FeaturedInRanking,
    WriteHonestReview,
    Write5HonestReview,
    Write10HonestReview,
    UseRAG3Days,
    // UseSpotlight3Days,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestInfo {
    pub name: String,
    pub status: bool,
}

pub async fn compute_quests(
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
    encryption_public_key: EncryptionPublicKey,
    identity_public_key: VerifyingKey,
) -> Result<Vec<(QuestType, QuestInfo)>, String> {
    let mut quests = Vec::new();

    // Add quests in the same order as QuestType enum

    // InstalledApp
    quests.push((
        QuestType::InstalledApp,
        QuestInfo {
            name: "InstalledApp".to_string(),
            status: true,
        },
    ));

    // CreateIdentity
    quests.push((
        QuestType::CreateIdentity,
        QuestInfo {
            name: "CreateIdentity".to_string(),
            status: match compute_create_identity_quest(
                db.clone(),
                node_name.clone(),
                encryption_public_key.clone(),
                identity_public_key.clone(),
            )
            .await
            {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing create identity quest: {}", e);
                    false
                }
            },
        },
    ));

    // DownloadFromStore
    quests.push((
        QuestType::DownloadFromStore,
        QuestInfo {
            name: "DownloadFromStore".to_string(),
            status: match compute_download_store_quest(db.clone()).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing download store quest: {}", e);
                    false
                }
            },
        },
    ));

    // ComeBack2Days
    quests.push((
        QuestType::ComeBack2Days,
        QuestInfo {
            name: "ComeBack2Days".to_string(),
            status: match compute_return_for_days_quest(db.clone(), 2).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing comeback 2 days quest: {}", e);
                    false
                }
            },
        },
    ));

    // ComeBack4Days
    quests.push((
        QuestType::ComeBack4Days,
        QuestInfo {
            name: "ComeBack4Days".to_string(),
            status: match compute_return_for_days_quest(db.clone(), 4).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing comeback 4 days quest: {}", e);
                    false
                }
            },
        },
    ));

    // ComeBack7Days
    quests.push((
        QuestType::ComeBack7Days,
        QuestInfo {
            name: "ComeBack7Days".to_string(),
            status: match compute_return_for_days_quest(db.clone(), 7).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing comeback 7 days quest: {}", e);
                    false
                }
            },
        },
    ));

    // CreateTool
    quests.push((
        QuestType::CreateTool,
        QuestInfo {
            name: "CreateTool".to_string(),
            status: match compute_create_tool_quest(db.clone()).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing create tool quest: {}", e);
                    false
                }
            },
        },
    ));

    // SubmitAndGetApprovalForTool
    quests.push((
        QuestType::SubmitAndGetApprovalForTool,
        QuestInfo {
            name: "SubmitAndGetApprovalForTool".to_string(),
            status: match compute_submit_approval_quest(db.clone(), node_name.clone()).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing submit approval quest: {}", e);
                    false
                }
            },
        },
    ));

    // SubmitAndGetApprovalFor2Tool
    quests.push((
        QuestType::SubmitAndGetApprovalFor2Tool,
        QuestInfo {
            name: "SubmitAndGetApprovalFor2Tool".to_string(),
            status: match compute_submit_approval_quest_with_count(db.clone(), node_name.clone(), 2).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing submit approval 2 tools quest: {}", e);
                    false
                }
            },
        },
    ));

    // SubmitAndGetApprovalFor3Tool
    quests.push((
        QuestType::SubmitAndGetApprovalFor3Tool,
        QuestInfo {
            name: "SubmitAndGetApprovalFor3Tool".to_string(),
            status: match compute_submit_approval_quest_with_count(db.clone(), node_name.clone(), 3).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing submit approval 3 tools quest: {}", e);
                    false
                }
            },
        },
    ));

    // FeaturedInRanking
    quests.push((
        QuestType::FeaturedInRanking,
        QuestInfo {
            name: "FeaturedInRanking".to_string(),
            status: match compute_top_ranking_quest(db.clone(), node_name.clone()).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing featured in ranking quest: {}", e);
                    false
                }
            },
        },
    ));

    // WriteHonestReview
    quests.push((
        QuestType::WriteHonestReview,
        QuestInfo {
            name: "WriteHonestReview".to_string(),
            status: match compute_write_app_reviews_quest_with_count(db.clone(), node_name.clone(), 1).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing write review quest: {}", e);
                    false
                }
            },
        },
    ));

    // Write5HonestReview
    quests.push((
        QuestType::Write5HonestReview,
        QuestInfo {
            name: "Write5HonestReview".to_string(),
            status: match compute_write_app_reviews_quest_with_count(db.clone(), node_name.clone(), 5).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing write 5 reviews quest: {}", e);
                    false
                }
            },
        },
    ));

    // Write10HonestReview
    quests.push((
        QuestType::Write10HonestReview,
        QuestInfo {
            name: "Write10HonestReview".to_string(),
            status: match compute_write_app_reviews_quest_with_count(db.clone(), node_name.clone(), 10).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing write 10 reviews quest: {}", e);
                    false
                }
            },
        },
    ));

    // UseRAG3Days
    quests.push((
        QuestType::UseRAG3Days,
        QuestInfo {
            name: "UseRAG3Days".to_string(),
            status: match compute_use_rag_quest(db.clone()).await {
                Ok(status) => status,
                Err(e) => {
                    eprintln!("Error computing use RAG quest: {}", e);
                    false
                }
            },
        },
    ));

    // // UseSpotlight3Days
    // quests.push((
    //     QuestType::UseSpotlight3Days,
    //     QuestInfo {
    //         name: "UseSpotlight3Days".to_string(),
    //         status: compute_use_spotlight_quest(db.clone(), Utc::now())?.status == QuestStatus::Completed,
    //     },
    // ));

    Ok(quests)
}

pub async fn compute_create_identity_quest(
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
    encryption_public_key: EncryptionPublicKey,
    identity_public_key: VerifyingKey,
) -> Result<bool, String> {
    // First check if the node name is localhost
    if node_name.to_string() == "@@localhost.sep-shinkai" {
        println!("Identity is localhost, quest not completed");
        return Ok(false);
    }

    // Get registry data
    let registry = ShinkaiRegistry::new(
        "https://sepolia.base.org",
        "0x425fb20ba3874e887336aaa7f3fab32d08135ba9",
        None,
    )
    .await
    .map_err(|e| format!("Failed to create registry: {}", e))?;

    let onchain_identity = match registry.get_identity_record(node_name.to_string(), Some(true)).await {
        Ok(identity) => identity,
        Err(e) => {
            println!("Identity not found in registry: {}", e);
            return Ok(false);
        }
    };

    // Hash our local keys
    let local_enc_hash = hex::encode(encryption_public_key.to_bytes());
    let local_sig_hash = hex::encode(identity_public_key.to_bytes());

    // The registry keys are already hex strings of the hashes, no need to decode and hash again
    let registry_enc_hash = onchain_identity.encryption_key;
    let registry_sig_hash = onchain_identity.signature_key;

    println!(
        "Node Identity Found:\nName: {}\nLocal Encryption Key Hash: {}\nLocal Signature Key Hash: {}\nRegistry Encryption Key Hash: {}\nRegistry Signature Key Hash: {}",
        node_name,
        local_enc_hash,
        local_sig_hash,
        registry_enc_hash,
        registry_sig_hash
    );

    // Compare the hex strings directly
    let keys_match = local_enc_hash == registry_enc_hash && local_sig_hash == registry_sig_hash;

    println!("Has valid identity: {}", keys_match);
    Ok(keys_match)
}

pub async fn compute_download_store_quest(db: Arc<SqliteManager>) -> Result<bool, String> {
    // Get the list of default tools from the store
    let url = std::env::var("SHINKAI_TOOLS_DIRECTORY_URL")
        .unwrap_or_else(|_| "https://store-api.shinkai.com/store/defaults".to_string());

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
    let end_date = chrono::DateTime::parse_from_rfc3339("2025-04-18T23:59:59Z")
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

pub async fn compute_write_app_reviews_quest(db: Arc<SqliteManager>, node_name: ShinkaiName) -> Result<bool, String> {
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
        .with_timezone(&chrono::Local);
    let end_date = chrono::DateTime::parse_from_rfc3339("2025-03-06T23:59:59Z")
        .map_err(|e| format!("Failed to parse end date: {}", e))?
        .with_timezone(&chrono::Local);

    // Collect unique dates when jobs with file resources were created
    let mut unique_dates = std::collections::HashSet::new();

    for job in all_jobs {
        // Parse the job's creation date
        let job_date = chrono::DateTime::parse_from_rfc3339(&job.datetime_created())
            .map_err(|e| format!("Failed to parse job date: {}", e))?
            .with_timezone(&chrono::Local); // Convert to local timezone

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

/// Generates a cryptographic proof that combines a node signature, a payload, and a secret desktop key.
/// This function creates a deterministic signature that can be used to verify the authenticity of the payload
/// and its association with a specific node.
///
/// # Arguments
///
/// * `node_signature` - A unique identifier string for the node that will be used to deterministically derive an
///   Ed25519 key pair. eg the signature public key of the node
/// * `payload` - The data to be included in the proof, typically a JSON string containing relevant information
///
/// # Returns
///
/// Returns a Result containing a tuple of:
/// * A hex-encoded Ed25519 signature of the final proof
/// * A concatenated string containing the public key, a verification tag, and the base64-encoded payload
///
/// # Internal Process
///
/// 1. Uses a secret desktop key (from env or default) as an additional security factor
/// 2. Deterministically derives an Ed25519 key pair from the node signature
/// 3. Combines the public key with the secret desktop key to create a verification tag
/// 4. Constructs a proof string containing the public key, tag, and encoded payload
/// 5. Signs the proof with the derived private key
///
/// # Security Notes
///
/// * The function uses deterministic key generation which should be used with caution in cryptographic contexts
/// * The security relies on both the node signature and the secret desktop key
/// * The proof can be verified using the public key contained in the concatenated string
pub fn generate_proof(node_signature: String, payload: String) -> Result<(String, String), String> {
    // Get the secret desktop key from environment or use default
    // This adds an additional secret factor to the proof generation
    let secret_desktop_key: String =
        std::env::var("SECRET_DESKTOP_INSTALLATION_PROOF_KEY").unwrap_or_else(|_| "Dc9{3R9JmXe7£w9Fs](7".to_string());

    // Hash the node signature and take first 4 bytes to create a deterministic seed
    // This ensures the same node signature always generates the same key pair
    let mut hasher = Hasher::new();
    hasher.update(node_signature.as_bytes());
    let hash_result = hasher.finalize();
    let bytes: [u8; 4] = hash_result.as_bytes()[..4].try_into().unwrap();
    let deterministic_seed = u32::from_le_bytes(bytes);

    // Generate a deterministic Ed25519 key pair from the seed
    let (secret_key, public_key) = unsafe_deterministic_signature_keypair(deterministic_seed);
    // Convert the public key to hex for inclusion in the proof
    let public_key_hex = hex::encode(public_key.to_bytes());

    // Create a verification tag by combining the public key and secret desktop key
    // This tag helps verify the proof's authenticity and association with the desktop installation
    let combined = format!("{}{}", public_key_hex, secret_desktop_key);

    // Hash the combined value and take the last 8 characters as a verification tag
    let mut hasher = Hasher::new();
    hasher.update(combined.as_bytes());
    let hash_result = hasher.finalize();
    let hash_str = hex::encode(hash_result.as_bytes());
    let last_8_chars = &hash_str[hash_str.len() - 8..];

    // Construct the final proof string with three components:
    // 1. The public key (hex encoded)
    // 2. The verification tag (last 8 chars of the combined hash)
    // 3. The base64 encoded payload
    let concatenated = format!(
        "{}:::{}:::{}",
        public_key_hex,
        last_8_chars,
        base64::encode(payload.as_bytes())
    );

    // Create the final signature by:
    // 1. Hashing the concatenated proof string
    // 2. Signing the hash with the deterministic private key
    let mut hasher = Hasher::new();
    hasher.update(concatenated.as_bytes());
    let final_hash_result = hasher.finalize();
    let final_hash_bytes = final_hash_result.as_bytes();

    // Sign the final hash with the deterministic private key
    let signature = secret_key.sign(final_hash_bytes);

    // Return the hex-encoded signature and the concatenated proof string
    Ok((hex::encode(signature.to_bytes()), concatenated))
}

pub async fn compute_submit_approval_quest_with_count(
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
    required_count: usize,
) -> Result<bool, String> {
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

    // Check if we have enough approved apps
    Ok(apps.len() >= required_count)
}

pub async fn compute_write_app_reviews_quest_with_count(
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
    required_count: usize,
) -> Result<bool, String> {
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

    // Check if we have enough reviews
    Ok(reviews.len() >= required_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_proof() {
        let node_signature = "test_node_signature".to_string();
        let json_string = r#"{"number_of_qa_subscriptions":3, "number_of_subscriptions":5}"#.to_string();
        let result = generate_proof(node_signature, json_string.clone());
        assert!(result.is_ok());
        let expected_signature = "adfec30225d48079ba160b4ab1e30c0118ad39f18da9ee409da103bc17dd8fd556d7e0f91677a4fcd39f10bbf310c31d24dba502b096f1559786be3b65978f08";
        let expected_concatenated = "74255b58dd17b859e777177062510a821bb658b6bcfbe66071422e4240bbf702:::976ebc9b:::eyJudW1iZXJfb2ZfcWFfc3Vic2NyaXB0aW9ucyI6MywgIm51bWJlcl9vZl9zdWJzY3JpcHRpb25zIjo1fQ==";
        let (signature, concatenated) = result.unwrap();
        assert_eq!(signature, expected_signature);
        assert_eq!(concatenated, expected_concatenated);
    }
}
