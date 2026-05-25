/// Sandbox: PathGuard + Permission Model + Process Isolation
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Cross-platform path helpers ──

fn home_dir() -> PathBuf {
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Default".into())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
    };
    PathBuf::from(home)
}

fn data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join("AppData").join("Roaming"))
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library").join("Application Support")
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".local").join("share"))
    }
}

fn default_allowed_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    let data = data_dir();

    vec![
        data.join("ai-tools-hub"),           // app config
        home.join("Desktop"),                 // desktop
        home.join("Documents"),               // documents
        home.join("Downloads"),               // downloads
        // Windows-specific vault path
        { std::env::var("AI_HUB_VAULT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(if cfg!(target_os = "windows") { r"D:\我的工作台" } else { "/tmp/ai-hub-vault" })) },
    ]
}

// ── Permission Level ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionLevel {
    #[serde(rename = "restricted")]
    Restricted,
    #[serde(rename = "full-access")]
    FullAccess,
}

impl Default for PermissionLevel {
    fn default() -> Self { PermissionLevel::Restricted }
}

// ── PathGuard ──

pub struct PathGuard {
    allowed_prefixes: Vec<PathBuf>,
    denied_patterns: Vec<String>,
}

impl Default for PathGuard {
    fn default() -> Self {
        PathGuard {
            allowed_prefixes: default_allowed_dirs(),
            denied_patterns: vec![
                String::from("\\Windows\\System32"),
                String::from("\\Windows\\System"),
                String::from("\\Program Files"),
                String::from("\\Program Files (x86)"),
                String::from("\\.ssh"),
                String::from("/etc/shadow"),
                String::from("/etc/passwd"),
            ],
        }
    }
}

impl PathGuard {
    pub fn new(allowed: Vec<PathBuf>) -> Self {
        PathGuard {
            allowed_prefixes: allowed,
            denied_patterns: vec![
                String::from("\\Windows\\System32"),
                String::from("\\Windows\\System"),
                String::from("\\Program Files"),
                String::from("\\.ssh"),
                String::from("/etc/shadow"),
                String::from("/etc/passwd"),
            ],
        }
    }

    pub fn check(&self, path: &Path, level: &PermissionLevel) -> Result<(), String> {
        if *level == PermissionLevel::FullAccess {
            return Ok(());
        }

        let path_str = path.to_string_lossy().replace('/', "\\");

        for pattern in &self.denied_patterns {
            if pattern.contains('*') {
                let pat_parts: Vec<&str> = pattern.split('\\').collect();
                let path_parts: Vec<&str> = path_str.split('\\').collect();
                if pat_parts.len() <= path_parts.len() {
                    let mut matched = true;
                    for (i, part) in pat_parts.iter().enumerate() {
                        if *part == "*" { continue; }
                        if path_parts.get(i).map(|p| p.to_lowercase()) != Some(part.to_lowercase()) {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        return Err(format!("Path denied: matches '{}'", pattern));
                    }
                }
            } else if path_str.to_lowercase().contains(&pattern.to_lowercase()) {
                return Err(format!("Path denied: contains '{}'", pattern));
            }
        }

        for prefix in &self.allowed_prefixes {
            let prefix_str = prefix.to_string_lossy().replace('/', "\\");
            if path_str.to_lowercase().starts_with(&prefix_str.to_lowercase()) {
                return Ok(());
            }
        }

        Err(format!(
            "Path '{}' is outside allowed directories. Allowed: {:?}",
            path.display(),
            self.allowed_prefixes
        ))
    }

    pub fn check_read(&self, path: &Path) -> Result<(), String> {
        self.check(path, &PermissionLevel::Restricted)
    }

    pub fn allowed_dirs_debug(&self) -> Vec<String> {
        self.allowed_prefixes.iter().map(|p| p.to_string_lossy().to_string()).collect()
    }

    pub fn add_allowed(&mut self, path: PathBuf) {
        if !self.allowed_prefixes.contains(&path) {
            self.allowed_prefixes.push(path);
        }
    }
}

// ── Tool Permission Extension ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermissions {
    pub level: PermissionLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,
}

impl Default for ToolPermissions {
    fn default() -> Self {
        ToolPermissions {
            level: PermissionLevel::Restricted,
            allowed_paths: None,
        }
    }
}

// ── Windows Process Sandbox ──

#[cfg(target_os = "windows")]
pub mod windows_sandbox {
    use std::process::{Command, Stdio};
    use std::os::windows::process::CommandExt;

    pub fn restrict_process(cmd: &mut Command) {
        cmd.creation_flags(0x08000200);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        if let Ok(tmp) = std::env::var("TEMP") {
            cmd.current_dir(&tmp);
        }
    }

    pub fn run_sandboxed(command: &str, args: &[&str], input: Option<&str>, _timeout_secs: u64) -> Result<String, String> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        if let Some(text) = input { cmd.arg(text); }
        restrict_process(&mut cmd);
        let output = cmd.output().map_err(|e| format!("Sandbox error: {}", e))?;
        let c = String::from_utf8_lossy(&output.stdout).to_string();
        let e = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(if c.is_empty() && !e.is_empty() { e } else { c })
    }
}

#[cfg(not(target_os = "windows"))]
pub mod windows_sandbox {
    use std::process::Command;
    pub fn restrict_process(_cmd: &mut Command) {}
    pub fn run_sandboxed(command: &str, args: &[&str], input: Option<&str>, _timeout_secs: u64) -> Result<String, String> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        if let Some(text) = input { cmd.arg(text); }
        let output = cmd.output().map_err(|e| format!("Sandbox error: {}", e))?;
        let c = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(c)
    }
}
