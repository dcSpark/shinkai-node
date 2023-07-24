use lazy_static::lazy_static;
use std::io;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

lazy_static! {
    pub static ref DEFAULT_LOCAL_AI_PORT: &'static str = "7999";
}

pub struct LocalAIProcess {
    child: Child,
}

impl LocalAIProcess {
    /// Starts the LocalAI process, which gets killed if the
    /// the `LocalAIProcess` struct gets dropped.
    pub fn start() -> io::Result<LocalAIProcess> {
        let child = Command::new("./local-ai")
            .arg("--threads")
            .arg("8")
            .arg("--address")
            .arg(format!(":{}", DEFAULT_LOCAL_AI_PORT.to_string()))
            .spawn()?;

        // Wait for 1/10th of a second for the LocalAI process to boot up/initialize its
        // web server
        let duration = Duration::from_millis(100);
        thread::sleep(duration);
        Ok(LocalAIProcess { child })
    }
}

impl Drop for LocalAIProcess {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(_) => println!("Successfully killed the local-ai server process."),
            Err(e) => println!("Failed to kill the local-ai server process: {}", e),
        }
    }
}
