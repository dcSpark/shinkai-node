use lazy_static::lazy_static;
use std::fs::File;
use std::io;
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
        let dev_null = if cfg!(windows) {
            File::open("NUL").unwrap()
        } else {
            File::open("/dev/null").unwrap()
        };

        // Wait for the previous tests bert.cpp to close
        let start_time = Instant::now();
        let mut disconnected = false;
        while !disconnected && start_time.elapsed() < Duration::from_millis(500) {
            thread::sleep(Duration::from_millis(50)); // Wait before each attempt
            disconnected =
                TcpStream::connect(("localhost", DEFAULT_LOCAL_EMBEDDINGS_PORT.parse::<u16>().unwrap())).is_err();
        }

        if !disconnected {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Previous server did not close within 500ms",
            ));
        }

        let child = Command::new("./bert-cpp-server")
            .arg("--model")
            .arg("models/all-MiniLM-L12-v2.bin")
            .arg("--threads")
            .arg("6")
            .arg("--port")
            .arg(format!("{}", DEFAULT_LOCAL_EMBEDDINGS_PORT.to_string()))
            .stdout(Stdio::from(dev_null.try_clone().unwrap())) // Redirect stdout
            .stderr(Stdio::from(dev_null)) // Redirect stderr
            .spawn()?;

        // Wait for for the BertCPP process to boot up/initialize its
        // web server
        let start_time = Instant::now();
        let mut connected = false;
        while !connected && start_time.elapsed() < Duration::from_millis(500) {
            thread::sleep(Duration::from_millis(50)); // Wait before each attempt
            connected =
                TcpStream::connect(("localhost", DEFAULT_LOCAL_EMBEDDINGS_PORT.parse::<u16>().unwrap())).is_ok();
        }

        if !connected {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Could not connect to the server within 500ms",
            ));
        }

        Ok(BertCPPProcess { child })
    }
}

impl Drop for BertCPPProcess {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(_) => {
                // Wait for the BertCPP process to close
                let start_time = Instant::now();
                let mut disconnected = false;
                while !disconnected && start_time.elapsed() < Duration::from_millis(500) {
                    thread::sleep(Duration::from_millis(50)); // Wait before each attempt
                    disconnected =
                        TcpStream::connect(("localhost", DEFAULT_LOCAL_EMBEDDINGS_PORT.parse::<u16>().unwrap()))
                            .is_err();
                }

                if !disconnected {
                    println!("Warning: The server did not close within 500ms");
                }
                println!("Successfully killed the bert-cpp server process.");
            }
            Err(e) => println!("Failed to kill the bert-cpp server process: {}", e),
        }
    }
}
