use std::collections::HashMap;
use anyhow::Result;
use baml_runtime::BamlRuntime;
use baml_types::BamlValue;
use indexmap::IndexMap;
use log::info;

#[derive(Clone, Debug)]
pub struct GeneratorConfig {
    pub output_type: String,
    pub output_dir: String,
    pub version: String,
    pub default_client_mode: String,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            output_type: "typescript".to_string(),
            output_dir: "../src/".to_string(),
            version: "0.55.3".to_string(),
            default_client_mode: "async".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub default_role: String,
}

#[derive(Clone, Debug)]
pub struct BamlConfig {
    pub generator: GeneratorConfig,
    pub client: ClientConfig,
    pub dsl_class_file: Option<String>,
    pub input: Option<String>,
    pub function_name: Option<String>,
    pub param_name: Option<String>,
}

impl BamlConfig {
    pub fn builder(generator: GeneratorConfig, client: ClientConfig) -> BamlConfigBuilder {
        BamlConfigBuilder {
            generator,
            client,
            dsl_class_file: None,
            input: None,
            function_name: None,
            param_name: None,
        }
    }

    pub fn initialize_runtime(&self, env_vars: HashMap<&str, &str>) -> Result<BamlRuntime> {
        let mut files = HashMap::new();
        files.insert("generator.baml".to_string(), format!(
            r##"
            generator lang_ts {{
                output_type "{}"
                output_dir "{}"
                version "{}"
                default_client_mode "{}"
            }}
            "##,
            self.generator.output_type,
            self.generator.output_dir,
            self.generator.version,
            self.generator.default_client_mode
        ));

        files.insert("client.baml".to_string(), format!(
            r##"
            client<llm> {} {{
                provider {}
                options {{
                    base_url "{}"
                    model "{}"
                    default_role "{}"
                }}
            }}
            "##,
            "ShinkaiProvider",
            self.client.provider,
            self.client.base_url,
            self.client.model,
            self.client.default_role
        ));

        if let Some(dsl_class_file) = &self.dsl_class_file {
            files.insert("dsl_class.baml".to_string(), dsl_class_file.clone());
        }

        info!("Files: {:?}", files);

        let runtime = BamlRuntime::from_file_content("baml_src", &files, env_vars)?;
        info!("BAML runtime initialized");

        Ok(runtime)
    }

    pub fn execute(&self, runtime: &BamlRuntime) -> Result<String> {
        let ctx_manager = runtime.create_ctx_manager(BamlValue::String("none".to_string()), None);

        let mut params = IndexMap::new();
        if let (Some(param_name), Some(input)) = (&self.param_name, &self.input) {
            params.insert(param_name.clone(), BamlValue::String(input.clone()));
        }

        if let Some(function_name) = &self.function_name {
            let (result, _uuid) = runtime.call_function_sync(
                function_name.clone(),
                &params,
                &ctx_manager,
                None,
                None,
            );

            match result {
                Ok(response) => {
                    match response.content() {
                        Ok(content) => {
                            let sanitized_content = content.replace("```", "");
                            info!("Function response: {:?}", sanitized_content);
                            return Ok(sanitized_content);
                        }
                        Err(e) => return Err(anyhow::anyhow!("Error getting content: {}", e)),
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Error: {}", e)),
            }
        }

        Err(anyhow::anyhow!("Function name not provided"))
    }
}

pub struct BamlConfigBuilder {
    generator: GeneratorConfig,
    client: ClientConfig,
    dsl_class_file: Option<String>,
    input: Option<String>,
    function_name: Option<String>,
    param_name: Option<String>,
}

impl BamlConfigBuilder {
    pub fn dsl_class_file(mut self, dsl_class_file: &str) -> Self {
        self.dsl_class_file = Some(dsl_class_file.to_string());
        self
    }

    pub fn input(mut self, input: &str) -> Self {
        self.input = Some(input.to_string());
        self
    }

    pub fn function_name(mut self, function_name: &str) -> Self {
        self.function_name = Some(function_name.to_string());
        self
    }

    pub fn param_name(mut self, param_name: &str) -> Self {
        self.param_name = Some(param_name.to_string());
        self
    }

    pub fn build(self) -> BamlConfig {
        BamlConfig {
            generator: self.generator,
            client: self.client,
            dsl_class_file: self.dsl_class_file,
            input: self.input,
            function_name: self.function_name,
            param_name: self.param_name,
        }
    }
}
