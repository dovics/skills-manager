use crate::error::SkillsError;
use anyhow::Result;
use std::path::Path;

/// Validate a skill directory structure
pub fn validate_skill(skill_path: &Path) -> Result<(), SkillsError> {
    if !skill_path.exists() {
        return Err(SkillsError::PathNotFound(
            skill_path.display().to_string(),
        ));
    }

    let skill_file = skill_path.join("SKILL.md");
    if !skill_file.exists() {
        return Err(SkillsError::ConfigError(
            format!("Invalid skill format: SKILL.md not found in {}", skill_path.display())
        ));
    }

    Ok(())
}
