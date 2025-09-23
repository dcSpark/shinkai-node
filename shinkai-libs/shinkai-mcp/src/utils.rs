use regex::Regex;
use std::collections::HashMap;

pub fn disect_command(command: String) -> (HashMap<String, String>, String, Vec<String>) {
    let mut cmd_parts = command.trim().split_whitespace();
    let mut env_vars = HashMap::new();
    let mut cmd_executable = "";
    let mut cmd_args = Vec::new();

    let env_regex = Regex::new(r#"^([A-Z0-9_]+)=(.*)$"#).unwrap();
    while let Some(part) = cmd_parts.next() {
        println!("part: {}", part);
        if let Some(captures) = env_regex.captures(part) {
            let key = captures[1].to_string();
            let mut value = captures[2].to_string();
            // Remove quotes if they exist at the start and end
            if (value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')) {
                value = value[1..value.len() - 1].to_string();
            }
            env_vars.insert(key, value);
        } else if cmd_executable.is_empty() {
            cmd_executable = part;
        } else {
            cmd_args.push(part.to_string());
        }
    }
    (env_vars, cmd_executable.to_string(), cmd_args)
}

#[cfg(test)]
pub mod tests_mcp_manager {
    use super::*;

    #[test]
    fn test_disect_command() {
        let cmd_str = "KEY_1=1 KEY_2=\"2\" binary_name -y mcp-server-org/mcp-server-repo";
        let (env_vars, cmd_executable, cmd_args) = disect_command(cmd_str.to_string());
        assert_eq!(env_vars.len(), 2);
        assert_eq!(env_vars["KEY_1"], "1");
        assert_eq!(env_vars["KEY_2"], "2");
        assert_eq!(cmd_executable, "binary_name");
        assert_eq!(cmd_args.len(), 2);
        assert_eq!(cmd_args[0], "-y");
        assert_eq!(cmd_args[1], "mcp-server-org/mcp-server-repo");
    }

    #[test]
    fn test_disect_command_with_no_envs() {
        let cmd_str = "binary_name -y mcp-server-org/mcp-server-repo";
        let (env_vars, cmd_executable, cmd_args) = disect_command(cmd_str.to_string());
        assert_eq!(env_vars.len(), 0);
        assert_eq!(cmd_executable, "binary_name");
        assert_eq!(cmd_args.len(), 2);
        assert_eq!(cmd_args[0], "-y");
        assert_eq!(cmd_args[1], "mcp-server-org/mcp-server-repo");
    }

    #[test]
    fn test_disect_command_with_quotes() {
        let cmd_str = "KEY_1=1 KEY_2='2' binary_name -y mcp-server-org/mcp-server-repo";
        let (env_vars, cmd_executable, cmd_args) = disect_command(cmd_str.to_string());
        assert_eq!(env_vars.len(), 2);
        assert_eq!(env_vars["KEY_1"], "1");
        assert_eq!(env_vars["KEY_2"], "2");
        assert_eq!(cmd_executable, "binary_name");
        assert_eq!(cmd_args.len(), 2);
        assert_eq!(cmd_args[0], "-y");
        assert_eq!(cmd_args[1], "mcp-server-org/mcp-server-repo");
    }
}
