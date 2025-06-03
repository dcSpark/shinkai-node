use std::pin::Pin;
use std::future::Future;
use tokio::task::AbortHandle;
use tokio::runtime::Runtime;
use async_channel::{Sender, Receiver, bounded};
use std::time::Duration;
use std::net::{SocketAddr, IpAddr, Ipv4Addr, TcpListener};
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use shinkai_node::network::Node;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair, hash_signature_public_key};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use ed25519_dalek::SigningKey;
use x25519_dalek::{StaticSecret as EncryptionStaticKey, PublicKey as EncryptionPublicKey};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;

pub struct TestContext {
    pub commands: Sender<NodeCommand>,
    pub abort_handle: AbortHandle,
    pub api_key: String,
    pub identity_name: String,
    pub profile_name: String,
    pub device_name: String,
    pub node_encryption_pk: EncryptionPublicKey,
    pub profile_encryption_sk: EncryptionStaticKey,
    pub profile_identity_sk: SigningKey,
    pub device_encryption_sk: EncryptionStaticKey,
    pub device_identity_sk: SigningKey,
}

#[derive(Clone, Default)]
pub struct TestConfig {
    pub openai_url: Option<String>,
}

impl TestConfig {
    pub fn default() -> Self {
        Self { openai_url: None }
    }

    pub fn with_mock_openai(mut self, url: impl Into<String>) -> Self {
        self.openai_url = Some(url.into());
        self
    }
}

pub fn run_test_one_node_network<F>(config: TestConfig, test: F)
where
    F: FnOnce(TestContext) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
{
    setup();
    setup_node_storage_path();
    let rt = Runtime::new().unwrap();

    if let Some(url) = &config.openai_url {
        std::env::set_var("OPENAI_API_URL", url);
    }

    fn port_is_available(port: u16) -> bool {
        TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    let status: anyhow::Result<()> = rt.block_on(async {
        let identity_name = "@@node1_test.sep-shinkai";
        let profile_name = "main";
        let device_name = "node1_device";

        let (identity_sk, identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (encryption_sk, encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (profile_identity_sk, _profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (profile_encryption_sk, _profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (device_identity_sk, _device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (device_encryption_sk, _device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (commands_sender, commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) = bounded(100);

        let node_db_path = format!("db_tests/{}", hash_signature_public_key(&identity_pk));

        let proxy_identity: Option<String> = env::var("PROXY_IDENTITY").ok().and_then(|addr| addr.parse().ok());

        let api_key = env::var("API_V2_KEY").unwrap_or_else(|_| "SUPER_SECRET".to_string());

        assert!(port_is_available(8080), "Port 8080 is not available");
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127,0,0,1)), 8080);
        let node = Node::new(
            identity_name.to_string(),
            addr,
            clone_signature_secret_key(&identity_sk),
            encryption_sk.clone(),
            None,
            None,
            0,
            commands_receiver.clone(),
            node_db_path,
            "".to_string(),
            proxy_identity,
            false,
            vec![],
            Some(RemoteEmbeddingGenerator::new_default()),
            None,
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM),
            vec![EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM)],
            Some(api_key.clone()),
        ).await;

        let abort_handle;
        {
            let node_clone = node.clone();
            let handler = tokio::spawn(async move {
                let _ = node_clone.lock().await.start().await;
            });
            abort_handle = handler.abort_handle();
        }

        let ctx = TestContext {
            commands: commands_sender.clone(),
            abort_handle,
            api_key: api_key.clone(),
            identity_name: identity_name.to_string(),
            profile_name: profile_name.to_string(),
            device_name: device_name.to_string(),
            node_encryption_pk: encryption_pk,
            profile_encryption_sk,
            profile_identity_sk,
            device_encryption_sk,
            device_identity_sk,
        };

        let user_fut = test(ctx);
        user_fut.await;
        Ok(())
    });
    rt.shutdown_timeout(Duration::from_secs(10));
    if let Err(e) = status { panic!("{:?}", e); }
    assert!(TcpListener::bind(("127.0.0.1", 8080)).is_ok(), "Port 8080 is not available");
}

impl TestContext {
    pub async fn register_device(&self) -> anyhow::Result<()> {
        use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType};
        use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;

        let (res_registration_sender, res_registration_receiver) = async_channel::bounded(1);
        self.commands
            .send(NodeCommand::LocalCreateRegistrationCode {
                permissions: IdentityPermissions::Admin,
                code_type: RegistrationCodeType::Device("main".to_string()),
                res: res_registration_sender,
            })
            .await?;
        let code = res_registration_receiver.recv().await?;

        let msg = ShinkaiMessageBuilder::use_code_registration_for_device(
            self.device_encryption_sk.clone(),
            clone_signature_secret_key(&self.device_identity_sk),
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            code.to_string(),
            RegistrationCodeType::Device("main".to_string()).to_string(),
            IdentityPermissions::Admin.to_string(),
            self.device_name.clone(),
            "".to_string(),
            self.identity_name.clone(),
            self.identity_name.clone(),
        ).map_err(|e| anyhow::anyhow!(e))?;

        let (res_use_sender, res_use_receiver) = async_channel::bounded(1);
        self.commands
            .send(NodeCommand::APIUseRegistrationCode { msg, res: res_use_sender })
            .await?;
        let result = res_use_receiver.recv().await?;
        result.map(|_| ()).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))
    }

    pub async fn register_llm_provider(&self, agent: SerializedLLMProvider) -> anyhow::Result<()> {
        use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;

        let (res_sender, res_receiver) = async_channel::bounded(1);
        let msg = ShinkaiMessageBuilder::request_add_llm_provider(
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            agent.clone(),
            self.profile_name.clone(),
            self.identity_name.clone(),
            self.identity_name.clone(),
        ).map_err(|e| anyhow::anyhow!(e))?;
        self.commands
            .send(NodeCommand::APIAddAgent { msg, res: res_sender })
            .await?;
        res_receiver.recv().await?.map(|_| ()).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))
    }

    pub async fn create_job(&self, agent_sub: &str) -> anyhow::Result<String> {
        use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
        use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;

        let scope = MinimalJobScope::default();
        let msg = ShinkaiMessageBuilder::job_creation(
            scope,
            false,
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            self.identity_name.clone(),
            self.profile_name.clone(),
            self.identity_name.clone(),
            agent_sub.to_string(),
        ).map_err(|e| anyhow::anyhow!(e))?;

        let (res_sender, res_receiver) = async_channel::bounded(1);
        self.commands
            .send(NodeCommand::APICreateJob { msg, res: res_sender })
            .await?;
        let res = res_receiver.recv().await?;
        res.map_err(|e| anyhow::anyhow!(format!("{:?}", e)))
    }

    pub async fn send_job_message(&self, job_id: &str, msg_content: &str) -> anyhow::Result<()> {
        use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;

        let msg = ShinkaiMessageBuilder::job_message(
            job_id.to_string(),
            msg_content.to_string(),
            vec![],
            "".to_string(),
            self.profile_encryption_sk.clone(),
            clone_signature_secret_key(&self.profile_identity_sk),
            self.node_encryption_pk,
            self.identity_name.clone(),
            self.profile_name.clone(),
            self.identity_name.clone(),
            format!("{}/agent/{}", self.profile_name, self.device_name),
        ).map_err(|e| anyhow::anyhow!(e))?;

        let (res_sender, res_receiver) = async_channel::bounded(1);
        self.commands
            .send(NodeCommand::APIJobMessage { msg, res: res_sender })
            .await?;
        res_receiver.recv().await?.map(|_| ()).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))
    }

    pub async fn wait_for_response(&self, timeout: Duration) -> anyhow::Result<String> {
        let (res_sender, res_receiver) = async_channel::bounded(1);
        self.commands
            .send(NodeCommand::FetchLastMessages { limit: 1, res: res_sender })
            .await?;
        let msgs = res_receiver.recv().await?;
        let msg_hash = msgs[0].calculate_message_hash_for_pagination();
        let start = std::time::Instant::now();
        loop {
            let (res_sender, res_receiver) = async_channel::bounded(1);
            self.commands
                .send(NodeCommand::FetchLastMessages { limit: 2, res: res_sender })
                .await?;
            let msgs = res_receiver.recv().await?;
            if msgs.len() == 2 && msgs[1].calculate_message_hash_for_pagination() == msg_hash {
                let content = msgs[0].get_message_content().unwrap_or_default();
                return Ok(content);
            }
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("timeout"));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

fn setup_node_storage_path() {
    let temp_file = NamedTempFile::new().unwrap();

    let path = PathBuf::from(temp_file.path());
    let parent_path = path.parent().unwrap();

    std::env::set_var("NODE_STORAGE_PATH", parent_path);

    let base_path = ShinkaiPath::base_path();

    let _ = fs::remove_dir_all(base_path.as_path());
}
