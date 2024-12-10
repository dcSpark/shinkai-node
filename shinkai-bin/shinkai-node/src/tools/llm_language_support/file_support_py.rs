pub fn generate_file_support_py(declaration_only: bool) -> String {
    let function_definitions = vec![
        (
            "get_mount_paths",
            "Gets an array of mounted files.",
            "List[str]",
            vec![],
            "mount_paths = os.environ.get('SHINKAI_MOUNT')\n    if not mount_paths:\n        return []\n    return [path.strip() for path in mount_paths.split(',')]",
            "Array of files"
        ),
        (
            "get_asset_paths",
            "Gets an array of asset files. These files are read only.",
            "List[str]",
            vec![],
            "asset_paths = os.environ.get('SHINKAI_ASSETS')\n    if not asset_paths:\n        return []\n    return [path.strip() for path in asset_paths.split(',')]",
            "Array of files"
        ),
        (
            "get_home_path",
            "Gets the home directory path. All created files must be written to this directory.",
            "str",
            vec![],
            "return os.environ.get('SHINKAI_HOME', '')",
            "Home directory path"
        ),
        (
            "get_shinkai_node_location",
            "Gets the Shinkai Node location URL. This is the URL of the Shinkai Node server.",
            "str",
            vec![],
            "return os.environ.get('SHINKAI_NODE_LOCATION', '')",
            "Shinkai Node URL"
        ),
        (
            "get_access_token",
            "Gets a valid OAuth AccessToken for the given provider.",
            "str",
            vec!["provider_name: str"],
            r#"from typing import TypedDict, List, Optional

class ProviderConfig(TypedDict):
    name: str
    version: str
    authorizationUrl: str
    redirectUrl: str
    tokenUrl: str
    clientId: str
    clientSecret: str
    scopes: List[str]
    grantType: str
    refreshToken: Optional[str]
    accessToken: Optional[str]

oauth_config_str = os.environ.get('SHINKAI_OAUTH', '{}')
try:
    oauth_config: List[ProviderConfig] = json.loads(oauth_config_str)
    if not oauth_config:
        raise ValueError('OAuth configuration not defined. Fix tool configuration.')
        
    provider_config = next((config for config in oauth_config if config['name'] == provider_name), None)
    
    if not provider_config:
        raise ValueError(f'OAuth configuration not found for provider: {provider_name}')
    
    # Handle OAuth 1.0
    if provider_config.get('version') == '1.0' or provider_config.get('grantType') == 'authorization_code':
        return provider_config.get('accessToken', '')
        
    # Handle OAuth 2.0
    if provider_config.get('version') == '2.0':
        # Check for refresh token
        refresh_token = provider_config.get('refreshToken')
        if not refresh_token:
            raise ValueError(f'No refresh token found for provider: {provider_name}')
        
        # Make request to refresh token endpoint
        async with aiohttp.ClientSession() as session:
            async with session.post(
                provider_config['tokenUrl'],
                headers={'Content-Type': 'application/x-www-form-urlencoded'},
                data={
                    'grant_type': 'refresh_token',
                    'refresh_token': refresh_token,
                    'client_id': provider_config['clientId'],
                    'client_secret': provider_config['clientSecret']
                }
            ) as response:
                if response.status != 200:
                    raise ValueError(f'Failed to refresh token: {await response.text()}')
                    
                data = await response.json()
                return data['access_token']
        
    raise ValueError(f'Unsupported OAuth version for provider: {provider_name}')
except Exception as e:
    print(f'Error getting access token: {str(e)}')
    return ''"#,
            "OAuth access token"
        ),
    ];

    let mut output = String::new();

    if !declaration_only {
        output.push_str("import os\nimport json\nimport aiohttp\nfrom typing import List, TypedDict, Optional\n\n");
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
