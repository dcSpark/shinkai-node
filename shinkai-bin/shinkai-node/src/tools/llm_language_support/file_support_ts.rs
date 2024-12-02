pub fn generate_file_support_ts(declaration_only: bool) -> String {
    let function_definitions = vec![
        (
            "getMountPaths",
            "Gets an array of mounted files.",
            "string[]",
            "const mountPaths = Deno.env.get('MOUNT');\n    if (!mountPaths) return [];\n    return mountPaths.split(',').map(path => path.trim());"
        ),
        (
            "getAssetPaths",
            "Gets an array of asset files. These files are read only.",
            "string[]",
            "const assetPaths = Deno.env.get('ASSETS');\n    if (!assetPaths) return [];\n    return assetPaths.split(',').map(path => path.trim());"
        ),
        (
            "getHomePath",
            "Gets the home directory path. All created files must be written to this directory.",
            "string",
            "return Deno.env.get('HOME') || \"\";"
        ),
        (
            "getShinkaiNodeLocation",
            "Gets the Shinkai Node location URL. This is the URL of the Shinkai Node server.",
            "string",
            "return Deno.env.get('SHINKAI_NODE_LOCATION') || \"\";"
        ),
    ];

    let mut output = String::new();

    for (name, doc, return_type, implementation) in function_definitions {
        output.push_str(&format!(
            r#"
/**
 * {doc}
 * @returns {{{return_type}}} Array of files.
 */
"#
        ));

        if declaration_only {
            output.push_str(&format!("declare function {name}(): {return_type};\n"));
        } else {
            output.push_str(&format!(
                "export function {name}(): {return_type} {{\n    {implementation}\n}}\n"
            ));
        }
    }

    output
}
