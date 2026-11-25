use std::env;

#[derive(Debug, Clone)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy)]
pub struct AgentPreset {
    pub name: &'static str,
    pub command: &'static str,
}

pub const AGENT_PRESETS: &[AgentPreset] = &[
    AgentPreset {
        name: "Claude",
        command: "claude",
    },
    AgentPreset {
        name: "Gemini",
        command: "gemini",
    },
    AgentPreset {
        name: "Codex",
        command: "codex",
    },
    AgentPreset {
        name: "Aider",
        command: "aider",
    },
    AgentPreset {
        name: "OpenCode",
        command: "opencode",
    },
];

/// Detects the best interactive shell for the current OS (Windows or Linux).
pub fn default_shell() -> ShellCommand {
    #[cfg(windows)]
    {
        if is_on_path("pwsh.exe") {
            return ShellCommand {
                program: "pwsh.exe".to_string(),
                args: vec!["-NoLogo".to_string()],
                display_name: "pwsh".to_string(),
            };
        }
        if is_on_path("powershell.exe") {
            return ShellCommand {
                program: "powershell.exe".to_string(),
                args: vec!["-NoLogo".to_string()],
                display_name: "PowerShell".to_string(),
            };
        }
        ShellCommand {
            program: "cmd.exe".to_string(),
            args: Vec::new(),
            display_name: "cmd".to_string(),
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(shell) = env::var("SHELL") {
            if !shell.trim().is_empty() {
                let name = std::path::Path::new(&shell)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sh")
                    .to_string();
                return ShellCommand {
                    program: shell,
                    args: Vec::new(),
                    display_name: name,
                };
            }
        }
        for candidate in ["/bin/bash", "/bin/zsh", "/bin/sh"] {
            if std::path::Path::new(candidate).exists() {
                let name = std::path::Path::new(candidate)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sh")
                    .to_string();
                return ShellCommand {
                    program: candidate.to_string(),
                    args: Vec::new(),
                    display_name: name,
                };
            }
        }
        ShellCommand {
            program: "/bin/sh".to_string(),
            args: Vec::new(),
            display_name: "sh".to_string(),
        }
    }
}

#[cfg(windows)]
fn is_on_path(exe: &str) -> bool {
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            if dir.join(exe).is_file() {
                return true;
            }
        }
    }
    false
}
