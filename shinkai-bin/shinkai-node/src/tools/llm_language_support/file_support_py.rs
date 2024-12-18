pub fn generate_file_support_py(declaration_only: bool) -> String {
    let function_definitions = vec![
        (
            "get_mount_paths",
            "Gets an array of mounted files.",
            "List[str]",
            vec![],
            r#"mount_paths = os.environ.get('SHINKAI_MOUNT')
    if not mount_paths:
        return []
    return [path.strip() for path in mount_paths.split(',') if path.strip()]"#,
            "Array of files",
        ),
        (
            "get_asset_paths",
            "Gets an array of asset files. These files are read only.",
            "List[str]",
            vec![],
            r#"asset_paths = os.environ.get('SHINKAI_ASSETS')
    if not asset_paths:
        return []
    return [path.strip() for path in asset_paths.split(',') if path.strip()]"#,
            "Array of files",
        ),
        (
            "get_home_path",
            "Gets the home directory path. All created files must be written to this directory.",
            "str",
            vec![],
            "return os.environ.get('SHINKAI_HOME', '')",
            "Home directory path",
        ),
        (
            "get_shinkai_node_location",
            "Gets the Shinkai Node location URL. This is the URL of the Shinkai Node server.",
            "str",
            vec![],
            "return os.environ.get('SHINKAI_NODE_LOCATION', '')",
            "Shinkai Node URL",
        ),
        (
            "get_access_token",
            "Gets a valid OAuth AccessToken for the given provider.",
            "str",
            vec!["provider_name: str"],
            r#"from typing import TypedDict, List, Optional

    oauth_config_str = os.environ.get('SHINKAI_OAUTH', '{}')
    try:
        oauth_config = json.loads(oauth_config_str)
        if not oauth_config:
            raise ValueError('OAuth configuration not defined. Fix tool configuration.')
            
        provider_config = next((config for config in oauth_config if config['name'] == provider_name), None)
        
        if not provider_config:
            raise ValueError(f'OAuth configuration not found for provider: {provider_name}')
        
        return provider_config['accessToken'] or ''
    except Exception as e:
        print(f'Error getting access token: {str(e)}')
        return ''"#,
            "OAuth access token",
        ),
    ];

    let mut output = String::new();

    if !declaration_only {
        output.push_str("import os\nimport json\nfrom typing import List, TypedDict, Optional\n\n");
    }

    for (name, doc, return_type, args, implementation, return_desc) in function_definitions {
        let param_str = if args.is_empty() {
            "".to_string()
        } else {
            args.join(", ")
        };

        output.push_str(&format!(
            r#"
async def {name}({param_str}) -> {return_type}:
    """{doc}
    
    Returns:
        {return_type}: {return_desc}
    """
"#
        ));

        if declaration_only {
            output.push_str("    ...\n\n");
        } else {
            output.push_str(&format!("    {implementation}\n\n"));
        }
    }

    output
}
