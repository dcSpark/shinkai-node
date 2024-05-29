use assert_cmd::prelude::*;
use shinkai_vector_resources::file_parser::file_parser_types::TextGroup; // Add methods on commands
use std::{
    io::Cursor,
    process::{Child, Command},
};

#[test]
fn pdf_parser_cli_test() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("--parse-pdf=../files/shinkai_intro.pdf");

    assert!(cmd.output().unwrap().status.success());

    let output: Vec<TextGroup> = serde_json::from_reader(Cursor::new(cmd.output().unwrap().stdout))?;

    assert!(output.len() > 0);
    assert!(output[0].text.contains("Shinkai Network Manifesto"));

    Ok(())
}

// struct Server {
//     process: Child,
// }

// impl Drop for Server {
//     fn drop(&mut self) {
//         let _ = self.process.kill();
//     }
// }

// #[test]
// fn pdf_parser_api_test() -> Result<(), Box<dyn std::error::Error>> {
//     let _server = Server {
//         process: Command::cargo_bin("shinkai-side-executor")?
//             .arg("--address=0.0.0.0:8090")
//             .spawn()?,
//     };

//     let file = std::fs::File::open("../files/shinkai_intro.pdf")?;

//     let client = reqwest::blocking::Client::new();
//     let response = client
//         .post("http://0.0.0.0:8090/v1/extract_json_to_text_groups/400")
//         .body(file)
//         .send()?
//         .json::<Vec<TextGroup>>()?;

//     assert!(response.len() > 0);
//     assert!(response[0].text.contains("Shinkai Network Manifesto"));

//     Ok(())
// }
