pub fn generate_file_support_ts(declaration_only: bool) -> String {
    let function_definitions = vec![
            (
                "getMountPaths",
                "Gets an array of mounted files.",
                "Promise<string[]>",
                vec![],
                "const mountPaths = Deno.env.get('SHINKAI_MOUNT');\n    if (!mountPaths) return [];\n    return mountPaths.split(',').map(path => path.trim());",
                "Array of files"
            ),
            (
                "getAssetPaths",
                "Gets an array of asset files. These files are read only.",
                "Promise<string[]>",
                vec![],
                "const assetPaths = Deno.env.get('SHINKAI_ASSETS');\n    if (!assetPaths) return [];\n    return assetPaths.split(',').map(path => path.trim());",
                "Array of files"
            ),
            (
                "getHomePath",
                "Gets the home directory path. All created files must be written to this directory.",
                "Promise<string>",
                vec![],
                "return Deno.env.get('SHINKAI_HOME') || \"\";",
                "Home directory path"
            ),
            (
                "getShinkaiNodeLocation",
                "Gets the Shinkai Node location URL. This is the URL of the Shinkai Node server.",
                "Promise<string>",
                vec![],
                "return Deno.env.get('SHINKAI_NODE_LOCATION') || \"\";",
                "Shinkai Node URL"
            ),
            (
                "getAccessToken",
                "Gets a valid OAuth AccessToken for the given provider.",
                "Promise<string>",
                vec!["providerName: string"],
                r#"
    type ProviderConfig = {
        name: string,
        version: string,
        authorizationUrl: string,
        redirectUrl: string,
        tokenUrl: string,
        clientId: string,
        clientSecret: string,
        scopes: string[],
        grantType: string,
        refreshToken?: string,
        accessToken?: string,
    }            
    const oauthConfig: ProviderConfig[] | undefined = JSON.parse(Deno.env.get('SHINKAI_OAUTH') || '{}');
    if (!oauthConfig) {
        throw new Error(`OAuth configuration not defined. Fix tool configuration.`);
    }
    const providerConfig: ProviderConfig = oauthConfig.find(config => config.name === providerName);
    if (!providerConfig) {
        throw new Error(`OAuth configuration not found for provider: ${providerName}`);
    }

    try {
        if (providerConfig.version === '1.0' || providerConfig.grantType === 'authorization_code') {
            return providerConfig.accessToken || '';
        }
        if (providerConfig.version === '2.0') {
            // Check if we have a refresh token
            const refreshToken = providerConfig.refreshToken
            if (!refreshToken) {
                throw new Error(`No refresh token found for provider: ${providerName}`);
            }

            // Make request to refresh token endpoint
            const response = await fetch(providerConfig.tokenUrl, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/x-www-form-urlencoded',
                },
                body: new URLSearchParams({
                    grant_type: 'refresh_token',
                    refresh_token: refreshToken,
                    client_id: providerConfig.clientId,
                    client_secret: providerConfig.clientSecret,
                }),
            });

            if (!response.ok) {
                throw new Error(`Failed to refresh token: ${response.statusText}`);
            }

            const data = await response.json();
            return data.access_token;
        }
        throw new Error(`Unsupported OAuth version for provider: ${providerName}`);
    } catch (error) {
        console.error('Error getting access token:', error);
        return '';
    }"#,
                "OAuth access token"
            ),
        ];

    let mut output = String::new();

    for (name, doc, return_type, args, implementation, return_desc) in function_definitions {
        output.push_str(&format!(
            r#"
/**
 * {doc}
 * @returns {return_type} - {return_desc}.
 */
"#
        ));

        let param_str = if args.is_empty() {
            "".to_string()
        } else {
            args.join(", ")
        };

        if declaration_only {
            output.push_str(&format!("declare async function {name}({param_str}): {return_type};\n"));
        } else {
            output.push_str(&format!(
                "export async function {name}({param_str}): {return_type} {{\n    {implementation}\n}}\n"
            ));
        }
    }

    output
}
