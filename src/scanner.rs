use crate::error::SkillsError;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInfo {
    pub name: String,
    pub path: PathBuf,
}

/// Scan a directory for skills
pub async fn scan_directory(path: &Path, recursive: bool) -> Result<Vec<SkillInfo>, SkillsError> {
    let mut skills = Vec::new();

    if !path.exists() {
        return Err(SkillsError::PathNotFound(path.display().to_string()));
    }

    let walker = if recursive {
        WalkDir::new(path).follow_links(false).into_iter()
    } else {
        WalkDir::new(path)
            .max_depth(1)
            .follow_links(false)
            .into_iter()
    };

    for entry in walker
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let skill_path = entry.path();

        // Skip the root directory
        if skill_path == path {
            continue;
        }

        // Check if this directory contains a skill (has SKILL.md)
        if skill_path.join("SKILL.md").exists() {
            let name = skill_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            skills.push(SkillInfo {
                name,
                path: skill_path.to_path_buf(),
            });
        }
    }

    Ok(skills)
}

/// Scan for skills in multiple directories
pub async fn scan_multiple(paths: &[PathBuf], recursive: bool) -> Result<Vec<SkillInfo>, SkillsError> {
    let mut all_skills = Vec::new();

    for path in paths {
        match scan_directory(path, recursive).await {
            Ok(skills) => all_skills.extend(skills),
            Err(e) => {
                tracing::warn!("Failed to scan {}: {}", path.display(), e);
            }
        }
    }

    Ok(all_skills)
}

/// Scan a directory for skills (synchronous version)
pub fn scan_directory_sync(path: &Path, recursive: bool) -> Result<Vec<SkillInfo>, SkillsError> {
    let mut skills = Vec::new();

    if !path.exists() {
        return Err(SkillsError::PathNotFound(path.display().to_string()));
    }

    let walker = if recursive {
        WalkDir::new(path).follow_links(false).into_iter()
    } else {
        WalkDir::new(path)
            .max_depth(1)
            .follow_links(false)
            .into_iter()
    };

    for entry in walker
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let skill_path = entry.path();

        // Skip the root directory
        if skill_path == path {
            continue;
        }

        // Check if this directory contains a skill (has SKILL.md)
        if skill_path.join("SKILL.md").exists() {
            let name = skill_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            skills.push(SkillInfo {
                name,
                path: skill_path.to_path_buf(),
            });
        }
    }

    Ok(skills)
}
