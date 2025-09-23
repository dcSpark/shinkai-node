use log::{error, info};
use std::{collections::HashMap, ffi::OsStr};
use tokio::process::Command;

use once_cell::sync::Lazy;

#[cfg(not(windows))]
static ENVS: Lazy<HashMap<String, String>> = Lazy::new(get_envs);

/*
    Inspired, adapted and extended from
    https://github.com/tauri-apps/fix-path-env-rs
*/
#[cfg(not(windows))]
pub fn get_envs() -> HashMap<String, String> {
    let default_shell = DEFAULT_SHELL.clone();

    let mut cmd = std::process::Command::new(default_shell);

    cmd.arg("-ilc")
        .arg("echo -n \"_SHELL_ENV_DELIMITER_\"; env; echo -n \"_SHELL_ENV_DELIMITER_\"; exit")
        // Disables Oh My Zsh auto-update thing that can block the process.
        .env("DISABLE_AUTO_UPDATE", "true");

    if let Some(home) = home::home_dir() {
        cmd.current_dir(home);
    }

    let output = cmd.output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let shell_env_delimiter = stdout.split("_SHELL_ENV_DELIMITER_").nth(1);

                if shell_env_delimiter.is_none() {
                    error!("shell env delimiter not found: {}", stdout);
                    return HashMap::new();
                }

                let shell_env_delimiter = shell_env_delimiter.unwrap();
                let mut env_vars = HashMap::new();
                for line in String::from_utf8_lossy(&strip_ansi_escapes::strip(shell_env_delimiter))
                    .split('\n')
                    .filter(|l| !l.is_empty())
                {
                    let mut s = line.splitn(2, '=');
                    if let (Some(var), Some(value)) = (s.next(), s.next()) {
                        env_vars.insert(var.to_string(), value.to_string());
                    }
                }
                env_vars
            } else {
                error!(
                    "error executing default shell to grab environment variables: {}",
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                );
                HashMap::new()
            }
        }
        Err(e) => {
            error!("error executing default shell to grab environment variables: {}", e);
            HashMap::new()
        }
    }
}

static BASE_OS_ENVS: Lazy<Option<HashMap<String, String>>> = Lazy::new(get_base_os_envs);
fn get_base_os_envs() -> Option<HashMap<String, String>> {
    let env_location = "/usr/bin/env";
    info!("grabbing environment variables from {}", env_location);
    let env_output = std::process::Command::new(env_location).output();
    match env_output {
        Ok(env_output) => {
            let env_output_str = String::from_utf8_lossy(&env_output.stdout);
            let env_lines = env_output_str.lines();
            let mut env_map = std::collections::HashMap::new();
            for line in env_lines {
                if let Some((key, value)) = line.split_once('=') {
                    env_map.insert(key.to_string(), value.to_string());
                }
            }
            info!(
                "environment variables grabbed: {:?}",
                &env_map.iter().collect::<HashMap<_, _>>()
            );
            Some(env_map)
        }
        Err(e) => {
            error!("error executing env to grab environment variables: {}", e);
            None
        }
    }
}

static DEFAULT_SHELL: Lazy<String> = Lazy::new(default_shell);

fn default_shell() -> String {
    info!("grabbing default shell");
    if cfg!(windows) {
        info!("windows detected, using powershell");
        "powershell".to_string()
    } else {
        info!("unix detected, grabbing shell from environment variables");
        let envs = BASE_OS_ENVS.as_ref().cloned();

        let shell_fallback = "/bin/bash";
        envs.unwrap_or_else(|| {
            error!("no shell found in environment variables,fallbac to default bash");
            HashMap::from([("SHELL".to_string(), shell_fallback.to_string())])
        })
        .get("SHELL")
        .unwrap_or(&shell_fallback.to_string())
        .to_string()
    }
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone)]
pub struct CommandWrappedInShellBuilder {
    program: String,
    args: Option<Vec<String>>,
    envs: Option<HashMap<String, String>>,
}

impl CommandWrappedInShellBuilder {
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            program: program.as_ref().to_string_lossy().to_string(),
            args: None,
            envs: None,
        }
    }

    pub fn new_with_args<S, I>(program: S, args: I) -> Self
    where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = S>,
    {
        Self {
            program: program.as_ref().to_string_lossy().to_string(),
            args: Some(
                args.into_iter()
                    .map(|s| s.as_ref().to_string_lossy().to_string())
                    .collect(),
            ),
            envs: None,
        }
    }

    pub fn envs<I, S>(&mut self, envs: I) -> &mut CommandWrappedInShellBuilder
    where
        I: IntoIterator<Item = (S, S)>,
        S: AsRef<OsStr>,
    {
        self.envs = Some(
            envs.into_iter()
                .map(|(k, v)| {
                    (
                        k.as_ref().to_string_lossy().to_string(),
                        v.as_ref().to_string_lossy().to_string(),
                    )
                })
                .collect(),
        );
        self
    }

    /// Appends arguments to the command.
    pub fn args<I, S>(&mut self, args: I) -> &mut CommandWrappedInShellBuilder
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args = Some(
            args.into_iter()
                .map(|s| s.as_ref().to_string_lossy().to_string())
                .collect(),
        );
        self
    }

    /// Appends an argument to the command.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut CommandWrappedInShellBuilder {
        match &mut self.args {
            Some(args_vec) => args_vec.push(arg.as_ref().to_string_lossy().to_string()),
            None => self.args = Some(vec![arg.as_ref().to_string_lossy().to_string()]),
        }
        self
    }

    fn wrapped_in_shell<S: AsRef<OsStr>>(program_with_args: S) -> Command {
        let shell = DEFAULT_SHELL.clone();
        let mut command = Command::new(shell);
        command.arg("-c").arg(program_with_args);

        #[cfg(not(windows))]
        if let Some(path) = ENVS.get("PATH") {
            command.env("PATH", path);
        }

        #[cfg(windows)]
        {
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }

    pub fn wrap_in_shell_as_values<Program, Args, Envs, EnvKey, EnvValue>(
        program: Program,
        args: Option<Args>,
        envs: Option<Envs>,
    ) -> (String, Vec<String>, HashMap<String, String>)
    where
        Program: AsRef<OsStr>,
        Args: IntoIterator<Item = Program>,
        EnvKey: AsRef<OsStr>,
        EnvValue: AsRef<OsStr>,
        Envs: IntoIterator<Item = (EnvKey, EnvValue)>,
    {
        let adapted_program: String = DEFAULT_SHELL.clone();
        let mut adapted_args: Vec<String> = Vec::new();
        let mut adapted_envs: HashMap<String, String> = HashMap::new();

        adapted_args.push("-c".to_string());

        #[cfg(not(windows))]
        if let Some(path) = ENVS.get("PATH") {
            adapted_envs.insert("PATH".to_string(), path.to_string());
        }

        if let Some(envs) = envs {
            for (key, value) in envs {
                adapted_envs.insert(
                    key.as_ref().to_string_lossy().to_string(),
                    value.as_ref().to_string_lossy().to_string(),
                );
            }
        }

        let command_with_args = if let Some(args) = args {
            program.as_ref().to_string_lossy().to_string()
                + " "
                + &args
                    .into_iter()
                    .map(|s| s.as_ref().to_string_lossy().to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
        } else {
            program.as_ref().to_string_lossy().to_string()
        };

        adapted_args.push(command_with_args);

        (adapted_program, adapted_args, adapted_envs)
    }

    pub fn build(self) -> Command {
        let command_with_args = if let Some(args) = self.args {
            self.program.clone() + " " + &args.join(" ")
        } else {
            self.program.clone()
        };

        Self::wrapped_in_shell(command_with_args)
    }
}
