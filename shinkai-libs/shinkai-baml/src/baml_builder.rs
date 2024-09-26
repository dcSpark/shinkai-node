use anyhow::Result;
use baml_runtime::BamlRuntime;
use baml_types::BamlValue;
use indexmap::IndexMap;
use log::info;
use regex::Regex;
use std::collections::HashMap;

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
        files.insert(
            "generator.baml".to_string(),
            format!(
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
            ),
        );

        files.insert(
            "client.baml".to_string(),
            format!(
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
            ),
        );

        if let Some(dsl_class_file) = &self.dsl_class_file {
            files.insert("dsl_class.baml".to_string(), dsl_class_file.clone());
        }

        info!("Files: {:?}", files);

        let runtime = BamlRuntime::from_file_content("baml_src", &files, env_vars)?;
        info!("BAML runtime initialized");

        Ok(runtime)
    }

    pub fn execute(&self, runtime: &BamlRuntime, extract_data: bool) -> Result<String> {
        let ctx_manager = runtime.create_ctx_manager(BamlValue::String("none".to_string()), None);

        let mut params = IndexMap::new();
        if let (Some(param_name), Some(input)) = (&self.param_name, &self.input) {
            let trimmed_input = input.trim();
            let context_value = if trimmed_input.starts_with('{') && trimmed_input.ends_with('}') {
                eprintln!("input is a json string: {}", trimmed_input);
                let unescaped_input = BamlConfig::unescape_json_string(trimmed_input);
                BamlConfig::from_serde_value(serde_json::from_str(&unescaped_input).unwrap())
            } else {
                BamlValue::String(trimmed_input.to_string())
            };
            params.insert(param_name.clone(), context_value);
        }

        if let Some(function_name) = &self.function_name {
            let (result, _uuid) = runtime.call_function_sync(function_name.clone(), &params, &ctx_manager, None, None);

            match result {
                Ok(response) => match response.content() {
                    Ok(content) => {
                        if extract_data {
                            eprintln!("Extracting data from response: {}", content);
                            if content.starts_with('{') && content.ends_with('}') {
                                return Ok(content.to_string());
                            }
                            let re = Regex::new(r"```(?:json)?\s*([\s\S]*?)\s*```").unwrap();
                            if let Some(captures) = re.captures(&content) {
                                if let Some(matched) = captures.get(1) {
                                    return Ok(matched.as_str().to_string());
                                }
                            }
                            return Err(anyhow::anyhow!("No JSON block found in the response"));
                        } else {
                            return Ok(content.to_string());
                        }
                    }
                    Err(e) => return Err(anyhow::anyhow!("Error getting content: {}", e)),
                },
                Err(e) => return Err(anyhow::anyhow!("Error: {}", e)),
            }
        }

        Err(anyhow::anyhow!("Function name not provided"))
    }

    pub fn from_serde_value(value: serde_json::Value) -> BamlValue {
        match value {
            serde_json::Value::Null => BamlValue::Null,
            serde_json::Value::Bool(b) => BamlValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    BamlValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    BamlValue::Float(f)
                } else {
                    panic!("Unexpected number type")
                }
            }
            serde_json::Value::String(s) => BamlValue::String(s),
            serde_json::Value::Array(arr) => {
                let baml_values = arr.into_iter().map(BamlConfig::from_serde_value).collect();
                BamlValue::List(baml_values)
            }
            serde_json::Value::Object(obj) => {
                let baml_map = obj
                    .into_iter()
                    .map(|(k, v)| (k, BamlConfig::from_serde_value(v)))
                    .collect();
                BamlValue::Map(baml_map)
            }
        }
    }

    pub fn unescape_json_string(json_str: &str) -> String {
        let re = Regex::new(r#"\\(.)"#).unwrap();
        re.replace_all(json_str, "$1").to_string()
    }

    /// Converts the existing DSL string to the format expected by Baml.
    pub fn convert_dsl_class_file(old_dsl: &str) -> String {
        // Define regex patterns for different escape sequences
        let re_triple_backslash_quote = Regex::new(r#"\\\\\\""#).unwrap(); // Matches \\\"
        let re_newline = Regex::new(r#"\\n"#).unwrap(); // Matches \n
        let re_quote = Regex::new(r#"\\""#).unwrap(); // Matches \"
        let re_backslash = Regex::new(r#"\\\\"#).unwrap(); // Matches \\

        // Perform replacements using regex in the correct order
        // 1. Replace triple backslashes followed by a quote (\\\\\") with an escaped quote (\\")
        let intermediate = re_triple_backslash_quote.replace_all(old_dsl, "\\\"");
        // 2. Replace escaped newlines (\\n) with actual newlines (\n)
        let intermediate = re_newline.replace_all(&intermediate, "\n");
        // 3. Replace escaped quotes (\\") with actual quotes (")
        let intermediate = re_quote.replace_all(&intermediate, "\"");
        // 4. Replace escaped backslashes (\\\\) with a single backslash (\\)
        let intermediate = re_backslash.replace_all(&intermediate, "\\");

        // Optionally, adjust other parts of the DSL as needed
        // For example, change client provider from Ollama to ShinkaiProvider
        let re_client = Regex::new(r#"client\s+\w+"#).unwrap();
        let adjusted = re_client.replace_all(&intermediate, "client ShinkaiProvider");

        adjusted.to_string()
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
