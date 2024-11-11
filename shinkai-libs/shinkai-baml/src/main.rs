mod baml_builder;

use std::collections::HashMap;
use std::sync::Once;

use anyhow::Result;
use baml_builder::{BamlConfig, ClientConfig, GeneratorConfig};
use log::info;

static INIT: Once = Once::new();

fn main() -> Result<()> {
    INIT.call_once(|| {
        env_logger::init();
    });
    info!("Starting Baml Runtime");

    let generator_config = GeneratorConfig {
        output_type: "typescript".to_string(),
        output_dir: "../src/".to_string(),
        version: "0.55.3".to_string(),
        default_client_mode: "async".to_string(),
    };

    let client_config = ClientConfig {
        provider: "ollama".to_string(),
        base_url: Some("http://localhost:11434/v1".to_string()),
        model: "llama3.1:8b-instruct-q4_1".to_string(),
        default_role: "user".to_string(),
        api_key: None,
    };

    let baml_config = BamlConfig::builder(generator_config, client_config)
        .dsl_class_file(
            r##"
            class Resume {
                name string
                email string
                experience string[]
                skills string[]
            }

            function ExtractResume(resume: string) -> Resume {
                client ShinkaiProvider
                prompt #"
                    Extract from this content:
                    {{ resume }}

                    {{ ctx.output_format }}
                "#
            }
            "##,
        )
        .input(
            r#"
            Name: Nico Arqueros
            123 Main St, Anytown, USA
            Email: john.doe@email.com
            Phone: (555) 123-4567

            Education:
            Bachelor of Science in Computer Science
            University of Technology, 2015-2019

            Work Experience:
            Software Engineer, Tech Corp
            June 2019 - Present
            - Developed and maintained web applications
            - Collaborated with cross-functional teams

            Skills:
            JavaScript, TypeScript, React, Node.js, Git
            "#,
        )
        .function_name("ExtractResume")
        .param_name("resume")
        .build();

    let env_vars = HashMap::new();
    let runtime = baml_config.initialize_runtime(env_vars)?;
    let result = baml_config.execute(&runtime, true)?;
    println!("Execution result: {}", result);

    Ok(())
}
