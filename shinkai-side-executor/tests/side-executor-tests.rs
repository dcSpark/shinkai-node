use assert_cmd::prelude::*;
use reqwest::multipart;
use shinkai_side_executor::{api, models::dto::VRPackContent};
use shinkai_vector_resources::{
    embedding_generator::RemoteEmbeddingGenerator,
    file_parser::file_parser_types::TextGroup,
    vector_resource::{VRKai, VRPack},
};
use std::{
    io::Cursor,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::Command,
};
use tokio::runtime::Runtime;

#[test]
fn cli_pdf_extract_to_text_groups() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("pdf")
        .arg("extract-to-text-groups")
        .arg("--file=../files/shinkai_intro.pdf");

    assert!(cmd.output().unwrap().status.success());

    let output: Vec<TextGroup> = serde_json::from_reader(Cursor::new(cmd.output().unwrap().stdout))?;

    assert!(output.len() > 0);
    assert!(output[0].text.contains("Shinkai Network Manifesto"));

    Ok(())
}

#[test]
fn cli_vrkai_generate_from_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrkai")
        .arg("generate-from-file")
        .arg("--file=../files/shinkai_intro.pdf");

    assert!(cmd.output().unwrap().status.success());

    let output = cmd.output().unwrap().stdout;
    let output = String::from_utf8(output).unwrap();
    let trimmed_output = output.trim();

    assert!(trimmed_output.len() > 0);
    assert!(VRKai::from_base64(&trimmed_output).is_ok());

    Ok(())
}

#[test]
fn cli_vrkai_view_contents() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrkai")
        .arg("view-contents")
        .arg("--file=../files/shinkai_intro.vrkai");

    assert!(cmd.output().unwrap().status.success());

    let _output: VRKai = serde_json::from_reader(Cursor::new(cmd.output().unwrap().stdout))?;

    Ok(())
}

#[test]
fn cli_vrpack_generate_from_files() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrpack")
        .arg("generate-from-files")
        .arg("--file=../files/shinkai_intro.pdf")
        .arg("--file=../files/shinkai_welcome.md");

    assert!(cmd.output().unwrap().status.success());

    let output = cmd.output().unwrap().stdout;
    let output = String::from_utf8(output).unwrap();
    let trimmed_output = output.trim();

    let vrpack = VRPack::from_base64(&trimmed_output).unwrap();

    vrpack.print_internal_structure(None);

    Ok(())
}

#[test]
fn cli_vrpack_generate_from_vrkais() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrpack")
        .arg("generate-from-vrkais")
        .arg("--file=../files/shinkai_intro.vrkai")
        .arg("--file=../files/zeko.vrkai");

    assert!(cmd.output().unwrap().status.success());

    let output = cmd.output().unwrap().stdout;
    let output = String::from_utf8(output).unwrap();
    let trimmed_output = output.trim();

    let vrpack = VRPack::from_base64(&trimmed_output).unwrap();

    vrpack.print_internal_structure(None);

    Ok(())
}

#[test]
fn cli_vrpack_add_vrkais() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrpack")
        .arg("add-vrkais")
        .arg("--file=../files/shinkai_intro.vrpack")
        .arg("--vrkai-file=../files/zeko.vrkai");

    assert!(cmd.output().unwrap().status.success());

    let output = cmd.output().unwrap().stdout;
    let output = String::from_utf8(output).unwrap();
    let trimmed_output = output.trim();

    let vrpack = VRPack::from_base64(&trimmed_output).unwrap();

    vrpack.print_internal_structure(None);

    Ok(())
}

#[test]
fn cli_vrpack_add_folder() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrpack")
        .arg("add-folder")
        .arg("--file=../files/shinkai_intro.vrpack")
        .arg("--folder-name=Shinkai folder");

    assert!(cmd.output().unwrap().status.success());

    let output = cmd.output().unwrap().stdout;
    let output = String::from_utf8(output).unwrap();
    let trimmed_output = output.trim();

    let vrpack = VRPack::from_base64(&trimmed_output).unwrap();

    vrpack.print_internal_structure(None);

    Ok(())
}

#[test]
fn cli_vrpack_view_contents() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("shinkai-side-executor")?;

    cmd.arg("vrpack")
        .arg("view-contents")
        .arg("--file=../files/shinkai_intro.vrpack");

    assert!(cmd.output().unwrap().status.success());

    let _output: VRPackContent = serde_json::from_reader(Cursor::new(cmd.output().unwrap().stdout))?;

    Ok(())
}

#[test]
fn api_pdf_extract_to_text_groups() -> Result<(), Box<dyn std::error::Error>> {
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
fn api_vrkai_generate_from_file() -> Result<(), Box<dyn std::error::Error>> {
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

#[test]
fn api_vrkai_view_contents() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        // Test valid VRKai
        let vrkai = std::fs::read_to_string("../files/shinkai_intro.vrkai").unwrap();
        let form = multipart::Form::new().part("encoded_vrkai", multipart::Part::text(vrkai));

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8090/v1/vrkai/view-contents")
            .multipart(form)
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());

        let _vrkai = response.json::<VRKai>().await.unwrap();

        // Test invalid VRKai
        let invalid_vrkai = "invalid_vrkai";
        let form = multipart::Form::new().part("encoded_vrkai", multipart::Part::text(invalid_vrkai));

        let response = client
            .post("http://127.0.0.1:8090/v1/vrkai/view-contents")
            .multipart(form)
            .send()
            .await
            .unwrap();

        assert!(response.status().is_client_error());

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}

#[test]
fn api_vrpack_generate_from_files() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        let generator = RemoteEmbeddingGenerator::new_default();

        let pdf_file = std::fs::read("../files/shinkai_intro.pdf").unwrap();
        let pdf_form_file = multipart::Part::bytes(pdf_file).file_name("shinkai_intro.pdf");

        let md_file = std::fs::read("../files/shinkai_welcome.md").unwrap();
        let md_form_file = multipart::Part::bytes(md_file).file_name("shinkai_welcome.md");

        let form = multipart::Form::new()
            .part("file", pdf_form_file)
            .part("file", md_form_file)
            .part(
                "embedding_model",
                multipart::Part::text(generator.model_type.to_string()),
            )
            .part("embedding_gen_url", multipart::Part::text(generator.api_url));

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8090/v1/vrpack/generate-from-files")
            .multipart(form)
            .send()
            .await
            .unwrap();

        let response = response.text().await.unwrap();
        let _vrpack = VRPack::from_base64(&response).unwrap();

        _vrpack.print_internal_structure(None);

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}

#[test]
fn api_vrpack_generate_from_vrkais() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        let vrkai_file = std::fs::read("../files/shinkai_intro.vrkai").unwrap();
        let vrkai_form_file = multipart::Part::bytes(vrkai_file).file_name("shinkai_intro.vrkai");

        let form = multipart::Form::new()
            .part("file", vrkai_form_file)
            .part("vrpack_name", multipart::Part::text("Shinkai intro"));

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8090/v1/vrpack/generate-from-vrkais")
            .multipart(form)
            .send()
            .await
            .unwrap();

        let response = response.text().await.unwrap();
        let _vrpack = VRPack::from_base64(&response).unwrap();

        _vrpack.print_internal_structure(None);

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}

#[test]
fn api_vrpack_add_vrkais() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        let vrpack = std::fs::read_to_string("../files/shinkai_intro.vrpack").unwrap();

        let vrkai = std::fs::read_to_string("../files/zeko.vrkai").unwrap();

        let form = multipart::Form::new()
            .part("encoded_vrpack", multipart::Part::text(vrpack))
            .part("encoded_vrkai", multipart::Part::text(vrkai));

        let client = reqwest::Client::new();
        let response = client
            .put("http://127.0.0.1:8090/v1/vrpack/add-vrkais")
            .multipart(form)
            .send()
            .await
            .unwrap();

        let response = response.text().await.unwrap();
        let _vrpack = VRPack::from_base64(&response).unwrap();

        _vrpack.print_internal_structure(None);

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}

#[test]
fn api_vrpack_add_folder() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        let vrpack = std::fs::read_to_string("../files/shinkai_intro.vrpack").unwrap();

        let form = multipart::Form::new()
            .part("encoded_vrpack", multipart::Part::text(vrpack))
            .part("folder_name", multipart::Part::text("Shinkai folder"));

        let client = reqwest::Client::new();
        let response = client
            .put("http://127.0.0.1:8090/v1/vrpack/add-folder")
            .multipart(form)
            .send()
            .await
            .unwrap();

        let response = response.text().await.unwrap();
        let _vrpack = VRPack::from_base64(&response).unwrap();

        _vrpack.print_internal_structure(None);

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}

#[test]
fn api_vrpack_view_contents() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let server_handle = tokio::spawn(async {
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);
            let _ = api::run_api(address).await;
        });

        let abort_handler = server_handle.abort_handle();

        // Test valid VRKai
        let vrpack = std::fs::read_to_string("../files/shinkai_intro.vrpack").unwrap();
        let form = multipart::Form::new().part("encoded_vrpack", multipart::Part::text(vrpack));

        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8090/v1/vrpack/view-contents")
            .multipart(form)
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());

        let _vrpack_content = response.json::<VRPackContent>().await.unwrap();

        // Test invalid VRKai
        let invalid_vrpack = "invalid_vrpack";
        let form = multipart::Form::new().part("encoded_vrpack", multipart::Part::text(invalid_vrpack));

        let response = client
            .post("http://127.0.0.1:8090/v1/vrpack/view-contents")
            .multipart(form)
            .send()
            .await
            .unwrap();

        assert!(response.status().is_client_error());

        abort_handler.abort();
    });
    rt.shutdown_background();

    Ok(())
}
