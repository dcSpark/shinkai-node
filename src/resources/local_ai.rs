use lazy_static::lazy_static;
use std::io;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

lazy_static! {
    pub static ref DEFAULT_LOCAL_EMBEDDINGS_PORT: &'static str = "7999";
}

pub struct BertCPPProcess {
    child: Child,
}

impl BertCPPProcess {
    /// Starts the BertCPP process, which gets killed if the
    /// the `BertCPPProcess` struct gets dropped.
    pub fn start() -> io::Result<BertCPPProcess> {
        let child = Command::new("./server")
            .arg("--model")
            .arg("models/all-MiniLM-L12-v2.bin")
            .arg("--threads")
            .arg("8")
            .arg("--port")
            .arg(format!("{}", DEFAULT_LOCAL_EMBEDDINGS_PORT.to_string()))
            .spawn()?;

        // Wait for 1/10th of a second for the BertCPP process to boot up/initialize its
        // web server
        let duration = Duration::from_millis(100);
        thread::sleep(duration);
        Ok(BertCPPProcess { child })
    }
}

impl Drop for BertCPPProcess {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(_) => println!("Successfully killed the local-ai server process."),
            Err(e) => println!("Failed to kill the local-ai server process: {}", e),
        }
    }
}
