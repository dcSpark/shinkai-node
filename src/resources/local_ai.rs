use std::io;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

pub struct LocalAIProcess {
    child: Child,
}

impl LocalAIProcess {
    pub fn start() -> io::Result<LocalAIProcess> {
        let child = Command::new("./local-ai").spawn()?;
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
