use crate::error::SkillsError;
use serde::Deserialize;
use std::path::Path;

const DEFAULT_REGISTRY: &str = "https://clawhub.ai/api/v1";

/// Search results from clawhub
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub slug: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub score: f64,
}

/// Skill metadata from clawhub
#[derive(Debug, Clone, Deserialize)]
pub struct SkillMetaResponse {
    #[serde(rename = "latestVersion")]
    pub latest_version: Option<LatestVersion>,
    pub skill: Option<SkillMeta>,
    pub moderation: Option<ModerationInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LatestVersion {
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillMeta {
    pub slug: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModerationInfo {
    #[serde(rename = "isMalwareBlocked")]
    pub is_malware_blocked: bool,
    #[serde(rename = "isSuspicious")]
    pub is_suspicious: bool,
}

/// Search for skills on clawhub
pub async fn search_skills(
    query: &str,
    limit: Option<usize>,
    registry: Option<&str>,
) -> Result<SearchResponse, SkillsError> {
    let registry_url = registry.unwrap_or(DEFAULT_REGISTRY);
    let client = reqwest::Client::new();

    let mut url = format!("{}/search", registry_url.trim_end_matches('/'));
    if let Some(l) = limit {
        url.push_str(&format!("?limit={}", l));
    }
    url.push_str(&format!("&q={}", urlencoding::encode(query)));

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to fetch search results: {}", e)))?;

    if !response.status().is_success() {
        return Err(SkillsError::ConfigError(format!(
            "Search failed with status: {}",
            response.status()
        )));
    }

    response
        .json::<SearchResponse>()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to parse search response: {}", e)))
}

/// Get skill metadata from clawhub
pub async fn get_skill_meta(
    slug: &str,
    registry: Option<&str>,
    token: Option<&str>,
) -> Result<SkillMetaResponse, SkillsError> {
    let registry_url = registry.unwrap_or(DEFAULT_REGISTRY);
    let client = reqwest::Client::new();

    let url = format!("{}/skills/{}", registry_url.trim_end_matches('/'), slug);

    let mut request = client
        .get(&url)
        .header("Accept", "application/json");

    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {}", t));
    }

    let response = request
        .send()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to fetch skill metadata: {}", e)))?;

    if !response.status().is_success() {
        return Err(SkillsError::ConfigError(format!(
            "Failed to get skill metadata: {}",
            response.status()
        )));
    }

    response
        .json::<SkillMetaResponse>()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to parse skill metadata: {}", e)))
}

/// Download a skill from clawhub
pub async fn download_skill(
    slug: &str,
    version: &str,
    registry: Option<&str>,
    token: Option<&str>,
) -> Result<Vec<u8>, SkillsError> {
    let registry_url = registry.unwrap_or(DEFAULT_REGISTRY);
    let client = reqwest::Client::new();

    let mut url = format!("{}/download", registry_url.trim_end_matches('/'));
    url.push_str(&format!("?slug={}", slug));
    url.push_str(&format!("&version={}", version));

    let mut request = client.get(&url);

    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {}", t));
    }

    let response = request
        .send()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to download skill: {}", e)))?;

    if !response.status().is_success() {
        return Err(SkillsError::ConfigError(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }

    response
        .bytes()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to read download: {}", e)))
        .map(|b| b.to_vec())
}

/// Extract a ZIP file to a directory
pub fn extract_zip(zip_bytes: &[u8], dest: &Path) -> Result<(), SkillsError> {
    use std::io::Cursor;

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| SkillsError::ConfigError(format!("Failed to read zip archive: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| SkillsError::ConfigError(format!("Failed to read zip entry: {}", e)))?;

        let path = dest.join(file.name());

        // Safety check: prevent zip slip
        if !path.starts_with(dest) {
            return Err(SkillsError::ConfigError(
                "Invalid zip file: path traversal detected".to_string(),
            ));
        }

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&path).map_err(|e| {
                SkillsError::ConfigError(format!("Failed to create directory: {}", e))
            })?;
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    SkillsError::ConfigError(format!("Failed to create directory: {}", e))
                })?;
            }
            let mut outfile = std::fs::File::create(&path).map_err(|e| {
                SkillsError::ConfigError(format!("Failed to create file: {}", e))
            })?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| {
                SkillsError::ConfigError(format!("Failed to write file: {}", e))
            })?;
        }
    }

    Ok(())
}

/// Check if a string is a valid clawhub slug
/// Slugs should be in format "skill-name" (no slashes)
pub fn is_valid_clawhub_slug(slug: &str) -> bool {
    if slug.is_empty() {
        return false;
    }

    // Check for path traversal attempts
    if slug.contains("..") || slug.contains('\\') {
        return false;
    }

    // Check if it starts with a dot (hidden file/directory)
    if slug.starts_with('.') {
        return false;
    }

    // Slugs should not contain slashes
    if slug.contains('/') {
        return false;
    }

    true
}

/// Check if a string looks like a clawhub URL
pub fn is_clawhub_url(input: &str) -> bool {
    input.contains("clawhub.ai") || input.starts_with("clawhub:")
}

/// Verify a ClawHub token and return the username
pub async fn verify_token(
    token: &str,
    registry: &str,
) -> Result<String, SkillsError> {
    #[derive(Debug, Deserialize)]
    struct WhoamiResponse {
        user: Option<UserInfo>,
    }

    #[derive(Debug, Deserialize)]
    struct UserInfo {
        handle: Option<String>,
    }

    let client = reqwest::Client::new();
    let url = format!("{}/whoami", registry.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to verify token: {}", e)))?;

    if !response.status().is_success() {
        return Err(SkillsError::ConfigError(format!(
            "Token verification failed: {}",
            response.status()
        )));
    }

    let whoami: WhoamiResponse = response
        .json()
        .await
        .map_err(|e| SkillsError::ConfigError(format!("Failed to parse response: {}", e)))?;

    whoami
        .user
        .and_then(|u| u.handle)
        .ok_or_else(|| SkillsError::ConfigError("No user handle in response".to_string()))
}

