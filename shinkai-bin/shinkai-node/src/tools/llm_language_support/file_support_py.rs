pub fn generate_file_support_py(declaration_only: bool) -> String {
    let function_definitions = vec![
        (
            "get_mount_paths",
            "Gets an array of mounted files.",
            "List[str]",
            "mount_paths = os.environ.get('SHINKAI_MOUNT')\n    if not mount_paths:\n        return []\n    return [path.strip() for path in mount_paths.split(',')]",
            "Array of files"
        ),
        (
            "get_asset_paths",
            "Gets an array of asset files. These files are read only.",
            "List[str]",
            "asset_paths = os.environ.get('SHINKAI_ASSETS')\n    if not asset_paths:\n        return []\n    return [path.strip() for path in asset_paths.split(',')]",
            "Array of files"
        ),
        (
            "get_home_path",
            "Gets the home directory path. All created files must be written to this directory.",
            "str",
            "return os.environ.get('SHINKAI_HOME', '')",
            "Home directory path"
        ),
        (
            "get_shinkai_node_location",
            "Gets the Shinkai Node location URL. This is the URL of the Shinkai Node server.",
            "str",
            "return os.environ.get('SHINKAI_NODE_LOCATION', '')",
            "Shinkai Node URL"
        ),
    ];

    let mut output = String::new();

    if !declaration_only {
        output.push_str("import os\nfrom typing import List\n\n");
    }

    for (name, doc, return_type, implementation, return_desc) in function_definitions {
        output.push_str(&format!(
            r#"
def {name}() -> {return_type}:
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
