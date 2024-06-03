use assert_cmd::prelude::*;
use reqwest::multipart;
use shinkai_side_executor::api;
use shinkai_vector_resources::{
    embedding_generator::RemoteEmbeddingGenerator, file_parser::file_parser_types::TextGroup, vector_resource::VRKai,
};
use std::{
    io::Cursor,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::Command,
};
use tokio::runtime::Runtime;

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

#[test]
fn pdf_extract_to_text_groups_api_test() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        let file = std::fs::read("../files/shinkai_intro.pdf").unwrap();
        let form_file = multipart::Part::bytes(file).file_name("shinkai_intro.pdf");
        let form = multipart::Form::new().part("file", form_file);

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8090/v1/pdf/extract-to-text-groups")
            .multipart(form)
            .send()
            .await
            .unwrap();
        let response = response.json::<Vec<TextGroup>>().await.unwrap();

        assert!(response.len() > 0);
        assert!(response[0].text.contains("Shinkai Network Manifesto"));

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}

#[test]
fn vrkai_generate_from_file_api_test() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        let generator = RemoteEmbeddingGenerator::new_default();

        let file = std::fs::read("../files/shinkai_intro.pdf").unwrap();
        let form_file = multipart::Part::bytes(file).file_name("shinkai_intro.pdf");

        let form = multipart::Form::new()
            .part("file", form_file)
            .part(
                "embedding_model",
                multipart::Part::text(generator.model_type.to_string()),
            )
            .part("embedding_gen_url", multipart::Part::text(generator.api_url));

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8090/v1/vrkai/generate-from-file")
            .multipart(form)
            .send()
            .await
            .unwrap();

        let response = response.text().await.unwrap();
        let _vrkai = VRKai::from_base64(&response).unwrap();

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}
