use crate::config::SkillConfig;
use crate::error::SkillsError;
use anyhow::Result;
use std::collections::HashMap;
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
        return Err(SkillsError::InvalidSkillFormat(
            skill_path.display().to_string(),
        ));
    }

    Ok(())
}

/// Calculate load order for skills based on dependencies
pub fn calculate_load_order(skills: &HashMap<String, SkillConfig>) -> Result<Vec<String>> {
    let mut in_degree: HashMap<String, i32> = HashMap::new();
    let mut adj_list: HashMap<String, Vec<String>> = HashMap::new();

    // Initialize
    for name in skills.keys() {
        in_degree.insert(name.clone(), 0);
        adj_list.insert(name.clone(), Vec::new());
    }

    // Build graph (no dependencies for now, so just return all skills)
    let mut result: Vec<String> = skills.keys().cloned().collect();
    result.sort();
    Ok(result)
}
