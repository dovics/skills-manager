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

    /// Managed skills
    #[serde(default)]
    pub skills: HashMap<String, SkillConfig>,
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

    /// Priority for loading (higher = first)
    #[serde(default = "default_priority")]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    /// Skill name
    pub name: String,

    /// Original path (where symlink will be created)
    pub path: PathBuf,

    /// Tool name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_priority() -> i32 {
    5
}

impl Default for Config {
    fn default() -> Self {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());

        Config {
            workspace: PathBuf::from(format!("{}/.skills/workspace", home)),
            tools: HashMap::new(),
            skills: HashMap::new(),
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
        PathBuf::from(format!("{}/.skills/workspace", home))
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
                priority: 5,
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

    /// Add a skill to the configuration
    pub fn add_skill(
        &mut self,
        name: &str,
        path: &Path,
        tool: Option<&str>,
    ) -> Result<(), SkillsError> {
        if self.skills.contains_key(name) {
            return Err(ConfigError(format!("Skill already exists: {}", name)));
        }

        self.skills.insert(
            name.to_string(),
            SkillConfig {
                name: name.to_string(),
                path: path.to_path_buf(),
                tool: tool.map(|s| s.to_string()),
            },
        );

        Ok(())
    }

    /// Remove a skill from the configuration
    pub fn remove_skill(&mut self, name: &str) -> Result<(), SkillsError> {
        self.skills
            .remove(name)
            .ok_or_else(|| ConfigError(format!("Skill not found: {}", name)))?;

        Ok(())
    }

    /// Get a skill by name
    pub fn get_skill(&self, name: &str) -> Result<SkillConfig, SkillsError> {
        self.skills
            .get(name)
            .cloned()
            .ok_or_else(|| SkillsError::SkillNotFound(name.to_string()))
    }

    /// List all skills
    pub fn list_skills(&self) -> Vec<SkillConfig> {
        let mut skills: Vec<_> = self.skills.values().cloned().collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    /// Get skills by tool
    pub fn get_skills_by_tool(&self, tool: &str) -> Vec<SkillConfig> {
        self.skills
            .values()
            .filter(|s| s.tool.as_deref() == Some(tool))
            .cloned()
            .collect()
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
        assert!(config.workspace.ends_with(".skills/workspace"));
        assert!(config.tools.is_empty());
        assert!(config.skills.is_empty());
    }

    #[test]
    fn test_add_tool() {
        let mut config = Config::default();
        config.add_tool("test-tool", Path::new("/tmp/test")).unwrap();
        assert!(config.tools.contains_key("test-tool"));
    }

    #[test]
    fn test_add_skill() {
        let mut config = Config::default();
        config
            .add_skill("test-skill", Path::new("/tmp/skill"), None)
            .unwrap();
        assert!(config.skills.contains_key("test-skill"));
    }
}
