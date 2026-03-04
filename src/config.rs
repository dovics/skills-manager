use crate::error::{SkillsError, SkillsError::ConfigError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Workspace directory where all skills are stored
    pub workspace: PathBuf,

    /// AI tools configuration
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,

    /// ClawHub configuration
    #[serde(default)]
    pub clawhub: ClawHubConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClawHubConfig {
    /// ClawHub API token
    #[serde(default)]
    pub token: Option<String>,

    /// ClawHub registry URL
    #[serde(default = "default_clawhub_registry")]
    pub registry: String,
}

fn default_clawhub_registry() -> String {
    "https://clawhub.ai/api/v1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool name (claude-code, openclaw, etc.)
    pub name: String,

    /// Path to the tool's skills directory
    pub path: PathBuf,

    /// Whether the tool is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SkillInfo {
    /// Skill name
    pub name: String,

    /// Path to the skill in workspace
    pub path: PathBuf,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());

        Config {
            workspace: PathBuf::from(format!("{}/.skills", home)),
            tools: HashMap::new(),
            clawhub: ClawHubConfig {
                token: None,
                registry: default_clawhub_registry(),
            },
        }
    }
}

impl Config {
    /// Get default config path
    pub fn default_path() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(format!("{}/.config/skills/config.yaml", home))
    }

    /// Get default workspace path
    pub fn default_workspace_path() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(format!("{}/.skills", home))
    }

    /// Load config from file
    pub fn load(path: Option<&str>) -> Result<Self, SkillsError> {
        let config_path = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            Self::default_path()
        };

        if !config_path.exists() {
            return Err(ConfigError(format!(
                "Config file not found: {}",
                config_path.display()
            )));
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| ConfigError(format!("Failed to read config: {}", e)))?;

        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// Save config to file
    pub fn save(&self, path: &Path) -> Result<(), SkillsError> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ConfigError(format!("Failed to create config directory: {}", e)))?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| ConfigError(format!("Failed to serialize config: {}", e)))?;

        fs::write(path, content)
            .map_err(|e| ConfigError(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Add a tool to the configuration
    pub fn add_tool(&mut self, name: &str, path: &Path) -> Result<(), SkillsError> {
        if self.tools.contains_key(name) {
            return Err(ConfigError(format!("Tool already exists: {}", name)));
        }

        self.tools.insert(
            name.to_string(),
            ToolConfig {
                name: name.to_string(),
                path: path.to_path_buf(),
                enabled: true,
            },
        );

        Ok(())
    }

    /// Remove a tool from the configuration
    pub fn remove_tool(&mut self, name: &str) -> Result<(), SkillsError> {
        self.tools
            .remove(name)
            .ok_or_else(|| ConfigError(format!("Tool not found: {}", name)))?;

        Ok(())
    }

    /// List all skills from workspace (dynamic scan)
    pub fn list_skills(&self) -> Vec<SkillInfo> {
        if !self.workspace.exists() {
            return Vec::new();
        }

        let mut skills = Vec::new();

        // Scan workspace for skill directories
        if let Ok(entries) = fs::read_dir(&self.workspace) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                // Only consider directories
                if !path.is_dir() {
                    continue;
                }

                // Check if it's a skill (has SKILL.md)
                if path.join("SKILL.md").exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        skills.push(SkillInfo {
                            name: name.to_string(),
                            path,
                        });
                    }
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    /// Get a skill by name from workspace
    pub fn get_skill(&self, name: &str) -> Option<SkillInfo> {
        let skill_path = self.workspace.join(name);

        if skill_path.exists() && skill_path.join("SKILL.md").exists() {
            Some(SkillInfo {
                name: name.to_string(),
                path: skill_path,
            })
        } else {
            None
        }
    }

    /// Get paths to scan for skills
    pub fn get_scan_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        for tool in self.tools.values() {
            if tool.enabled {
                paths.push(tool.path.clone());
            }
        }

        paths
    }

    /// Auto-discover tools and add them to config
    pub fn discover_tools(&mut self) -> Result<Vec<String>, SkillsError> {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut discovered = Vec::new();

        // Common tool paths
        let tool_paths = [
            ("claude-code", format!("{}/.claude/skills", home)),
            ("openclaw", format!("{}/.openclaw/skills", home)),
            ("zeroclaw", format!("{}/.zeroclaw/skills", home)),
            ("npx-skills", format!("{}/.agents/skills", home))
        ];

        for (name, path) in tool_paths {
            let path_buf = PathBuf::from(&path);
            if path_buf.exists() && !self.tools.contains_key(name) {
                self.add_tool(name, &path_buf)?;
                discovered.push(name.to_string());
            }
        }

        Ok(discovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.workspace.ends_with(".skills"));
        assert!(config.tools.is_empty());
    }

    #[test]
    fn test_add_tool() {
        let mut config = Config::default();
        config.add_tool("test-tool", Path::new("/tmp/test")).unwrap();
        assert!(config.tools.contains_key("test-tool"));
    }
}
