// mcp_process_manager.rs  — COMPILING VERSION
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;
use tokio::{process::Command, spawn};

use rmcp::{ServiceExt, transport::TokioChildProcess};
use rmcp::service::{RunningService, Peer, RoleClient};
use rmcp::model::Tool;

type MCPHandle = RunningService<RoleClient, ()>;        // long‑lived task + peer
type SharedHandle = Arc<MCPHandle>;                     // what we store / clone
type MCPPeer   = Peer<RoleClient>;                      // lightweight RPC peer

pub struct MCPProcessManager {
    clients: Arc<Mutex<HashMap<i64, SharedHandle>>>,
}

impl MCPProcessManager {
    fn new() -> Self {
        Self { clients: Arc::new(Mutex::new(HashMap::new())) }
    }

    // ------------------------------------------------------------ SPAWN

    pub async fn spawn_command_server(
        &self,
        id: i64,
        name: &str,
        cmd_str: &str,
    ) -> anyhow::Result<()> {
        // 1. Launch child (via shell)
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(cmd_str)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());

        // 2. Create transport + handshake
        let transport = TokioChildProcess::new(&mut cmd)?;
        let handle: MCPHandle = ().serve(transport).await?;
        let shared: SharedHandle = Arc::new(handle);
        let peer: MCPPeer = shared.peer().clone();

        // 3. OPTIONAL watcher skipped — calling `waiting()` would
        //    require ownership of the `RunningService`, which we keep in an
        //    `Arc`.  You can re‑add a watcher later by storing the `JoinHandle`
        //    elsewhere (e.g. as a separate field).

        // 4. Store the shared handle so the child stays alive Store the shared handle so the child stays alive
        self.clients.lock().unwrap().insert(id, shared);

        // 5. Log tools
        if let Ok(resp) = peer.list_tools(None).await {
            println!("🔗  '{}' connected – tools:", name);
            for t in resp.tools {
                println!("   • {} — {}", t.name, t.description);
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ PUBLIC API

    /// Get an RPC peer by server id
    pub fn peer(&self, id: i64) -> Option<MCPPeer> {
        self.clients
            .lock()
            .unwrap()
            .get(&id)
            .map(|h| h.peer().clone())
    }

    /// Convenience: list tools via stored peer
    pub async fn list_tools(&self, id: i64) -> anyhow::Result<Vec<Tool>> {
        let peer = self
            .peer(id)
            .ok_or_else(|| anyhow::anyhow!("No running MCP server with id {id}"))?;
        Ok(peer.list_tools(None).await?.tools)
    }

    /// Graceful shutdown — takes the Arc out of the map, then tries to
    /// unwrap it to obtain the `RunningService`.  If other strong refs
    /// still exist, we simply drop our Arc (child keeps running).
    pub async fn shutdown(&self, id: i64) -> anyhow::Result<()> {
        if let Some(arc) = self.clients.lock().unwrap().remove(&id) {
            match Arc::try_unwrap(arc) {
                Ok(handle) => {
                    // We now own the handle → can cancel & await
                    handle.cancel().await?;
                }
                Err(still_shared) => {
                    // Someone else still holds a ref; we can decide to ignore or log
                    eprintln!("MCP server {id} still referenced elsewhere; not cancelled");
                    drop(still_shared);
                }
            }
        }
        Ok(())
    }
}

/// Global singleton
pub static MCP_MANAGER: Lazy<MCPProcessManager> = Lazy::new(MCPProcessManager::new);
