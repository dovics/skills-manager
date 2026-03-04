use crate::error::SkillsError;
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// Move a skill from source to destination
pub async fn move_skill(source: &Path, dest: &Path) -> Result<()> {
    // Create parent directory if needed
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Check if destination already exists
    if dest.exists() {
        return Err(SkillsError::SkillAlreadyExists(
            dest.display().to_string(),
        )
        .into());
    }

    // Move the directory
    fs::rename(source, dest).with_context(|| {
        format!(
            "Failed to move skill from {} to {}",
            source.display(),
            dest.display()
        )
    })?;

    Ok(())
}

/// Create a symlink from source to destination
pub async fn create_symlink(source: &Path, dest: &Path) -> Result<()> {
    // Remove existing file/directory at destination
    if dest.exists() {
        // Backup existing files
        if dest.is_file() || dest.is_dir() {
            let backup_path = backup_path(dest);
            fs::rename(dest, &backup_path).with_context(|| {
                format!(
                    "Failed to backup existing file: {}",
                    dest.display()
                )
            })?;
            tracing::info!("Backed up existing file to: {}", backup_path.display());
        }

        // Remove existing symlink
        if dest.is_symlink() {
            fs::remove_file(dest).with_context(|| {
                format!("Failed to remove existing symlink: {}", dest.display())
            })?;
        }
    }

    // Create parent directory if needed
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Create the symlink
    symlink(source, dest).with_context(|| {
        format!(
            "Failed to create symlink from {} to {}",
            source.display(),
            dest.display()
        )
    })?;

    tracing::info!("Created symlink: {} -> {}", dest.display(), source.display());

    Ok(())
}

/// Remove a symlink
pub async fn remove_symlink(link: &Path) -> Result<()> {
    if !link.exists() {
        return Ok(());
    }

    if !link.is_symlink() {
        return Err(SkillsError::ConfigError(format!(
            "Path is not a symlink: {}",
            link.display()
        ))
        .into());
    }

    fs::remove_file(link)
        .with_context(|| format!("Failed to remove symlink: {}", link.display()))?;

    tracing::info!("Removed symlink: {}", link.display());

    Ok(())
}

/// Check if a path is a valid symlink to the expected target
pub async fn verify_symlink(link: &Path, expected_target: &Path) -> Result<bool> {
    if !link.exists() || !link.is_symlink() {
        return Ok(false);
    }

    let target = fs::read_link(link).with_context(|| {
        format!(
            "Failed to read symlink target: {}",
            link.display()
        )
    })?;

    // Resolve both paths for comparison
    let canonical_link = link.canonicalize().ok();
    let canonical_expected = expected_target.canonicalize().ok();

    Ok(canonical_link == canonical_expected || target == expected_target)
}

/// Generate a backup path for a file
fn backup_path(path: &Path) -> PathBuf {
    let mut backup = path.to_path_buf();

    if let Some(extension) = path.extension() {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = extension.to_string_lossy();
        backup.set_file_name(format!("{}.backup.{}", stem, ext));
    } else {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        backup.set_file_name(format!("{}.backup", name));
    }

    backup
}

/// Copy a skill directory (alternative to moving)
pub async fn copy_skill(source: &Path, dest: &Path) -> Result<()> {
    // Create parent directory if needed
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Check if destination already exists
    if dest.exists() {
        return Err(SkillsError::SkillAlreadyExists(
            dest.display().to_string(),
        )
        .into());
    }

    // Copy the directory
    fs_extra::dir::copy(
        source,
        dest,
        &fs_extra::dir::CopyOptions {
            content_only: true,
            ..Default::default()
        },
    )
    .with_context(|| {
        format!(
            "Failed to copy skill from {} to {}",
            source.display(),
            dest.display()
        )
    })?;

    Ok(())
}

/// Sync a skill: move to workspace and create symlink
pub async fn sync_skill(
    source: &Path,
    workspace: &Path,
    skill_name: &str,
) -> Result<PathBuf> {
    let dest = workspace.join(skill_name);

    // Move to workspace
    move_skill(source, &dest).await?;

    // Create symlink at original location
    create_symlink(&dest, source).await?;

    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_path() {
        let path = Path::new("/tmp/test.md");
        let backup = backup_path(path);
        assert!(backup.to_string_lossy().contains(".backup"));

        let path = Path::new("/tmp/test");
        let backup = backup_path(path);
        assert!(backup.to_string_lossy().contains(".backup"));
    }
}
