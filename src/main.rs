mod cli;
mod clawhub;
mod config;
mod error;
mod linker;
mod scanner;
mod skills;
mod tui;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use tracing::warn;

use cli::{Commands, SkillsArgs};
use config::Config;
use error::SkillsError;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = SkillsArgs::parse();

    // Load configuration
    let config = Config::load(args.config.as_deref()).unwrap_or_else(|e| {
        eprintln!(
            "{} Failed to load config: {}",
            "WARNING:".yellow().bold(),
            e
        );
        Config::default()
    });

    // Execute command
    match args.command {
        Commands::Init { force } => {
            cmd_init(force).await?;
        }
        Commands::Scan { path, recursive } => {
            cmd_scan(&config, path, recursive).await?;
        }
        Commands::Add { name, path, tool, keep } => {
            cmd_add(&config, name, path, tool, keep).await?;
        }
        Commands::Remove { name } => {
            cmd_remove(&config, name).await?;
        }
        Commands::List { detailed } => {
            cmd_list(&config, detailed).await?;
        }
        Commands::Sync { tool, dry_run } => {
            cmd_sync(&config, tool, dry_run).await?;
        }
        Commands::Link { dry_run } => {
            cmd_link(&config, dry_run).await?;
        }
        Commands::Unlink { name, dry_run } => {
            cmd_unlink(&config, name, dry_run).await?;
        }
        Commands::Verify => {
            cmd_verify(&config).await?;
        }
        Commands::Config { show_path } => {
            cmd_config(show_path).await?;
        }
        Commands::Tui => {
            cmd_tui().await?;
        }
    }

    Ok(())
}

async fn cmd_init(force: bool) -> Result<()> {
    println!("{}", "🚀 Initializing Skills Manager...".green().bold());

    let config_path = Config::default_path();
    let workspace_path = Config::default_workspace_path();

    // Check if already initialized
    if config_path.exists() && !force {
        println!(
            "{} {}",
            "✓".green(),
            "Already initialized. Use --force to reinitialize.".bold()
        );
        println!("  Config: {}", config_path.display());
        println!("  Workspace: {}", workspace_path.display());
        return Ok(());
    }

    // Create config
    let mut config = Config::default();

    // Auto-discover tools
    println!("{}", "🔍 Scanning for AI tools...".cyan().bold());
    let discovered = config.discover_tools()?;

    if !discovered.is_empty() {
        println!();
        println!("{}", "Found tools:".bold());
        for tool in &discovered {
            println!("  ✓ {}", tool.cyan());
        }
    } else {
        println!("  {}", "No tools found. You can add them manually.".dimmed());
    }

    // Save config
    config.save(&config_path)?;

    // Create workspace directory
    std::fs::create_dir_all(&workspace_path)?;

    // Create categories
    let categories = ["01-general", "02-languages", "03-frameworks", "04-utilities"];
    for category in &categories {
        std::fs::create_dir_all(workspace_path.join(category))?;
    }

    println!();
    println!("{}", "✅ Initialization complete!".green().bold());
    println!();
    println!("{}", "Created:".bold());
    println!("  📄 Config: {}", config_path.display());
    println!("  📁 Workspace: {}", workspace_path.display());
    println!();
    println!("{}", "Next steps:".bold());
    println!("  1. View or edit the config file:");
    println!("     {}", format!("skills config",).cyan());
    println!("  2. Launch TUI to manage skills:");
    println!("     {}", "skills tui".cyan());
    println!("  3. Or scan for existing skills:");
    println!("     {}", "skills scan --recursive".cyan());

    Ok(())
}

async fn cmd_scan(config: &Config, path: Option<String>, recursive: bool) -> Result<()> {
    println!("{}", "🔍 Scanning for skills...".cyan().bold());

    let scan_paths = if let Some(path) = path {
        vec![std::path::PathBuf::from(path)]
    } else {
        config.get_scan_paths()
    };

    let mut found_skills = Vec::new();

    for path in scan_paths {
        if !path.exists() {
            warn!("Path does not exist: {}", path.display());
            continue;
        }

        println!("  Scanning: {}", path.display());

        let skills = scanner::scan_directory(&path, recursive).await?;
        found_skills.extend(skills);
    }

    println!();
    println!("{}", "Found skills:".bold());
    for skill in &found_skills {
        println!(
            "  📦 {} ({})",
            skill.name.bold(),
            skill.path.display().to_string().dimmed()
        );
    }

    println!();
    println!(
        "Total: {} {}",
        found_skills.len(),
        if found_skills.len() == 1 {
            "skill"
        } else {
            "skills"
        }
    );

    Ok(())
}

async fn cmd_add(config: &Config, name: Option<String>, path: String, _tool: Option<String>, keep: bool) -> Result<()> {
    use colored::Colorize;

    // Check if path is a clawhub URL or slug
    if clawhub::is_clawhub_url(&path) {
        // Extract slug from URL or use the input directly
        let slug = if path.starts_with("clawhub:") {
            path.trim_start_matches("clawhub:").to_string()
        } else if path.contains("clawhub.ai") {
            // Extract just the skill name from URL like https://clawhub.ai/pskoett/self-improving-agent
            // The slug is the last part (skill name only)
            path.split('/')
                .last()
                .filter(|s| !s.is_empty())
                .unwrap_or(&path)
                .to_string()
        } else {
            path.clone()
        };

        if !clawhub::is_valid_clawhub_slug(&slug) {
            println!("{} Invalid clawhub slug: {}", "⚠️ ".yellow(), slug);
            return Ok(());
        }

        println!("{} ClawHub skill detected: {}", "🔍".cyan(), slug);
        println!("{} Installing from ClawHub...", "📦".cyan());
        return cmd_install_from_clawhub(config, &slug, name, keep).await;
    }

    // Check if path is a GitHub URL
    if path.starts_with("http://") || path.starts_with("https://") {
        if extract_repo_name_from_url(&path).is_some() {
            println!("{} GitHub repository detected: {}", "🔍".cyan(), path);
            println!("{} Installing from GitHub...", "📦".cyan());
            return cmd_install_from_github(config, &path, name, _tool, keep).await;
        }
    }

    let skill_path = std::path::PathBuf::from(&path);

    if !skill_path.exists() {
        return Err(SkillsError::PathNotFound(path).into());
    }

    // Auto-detect skill name if not provided
    let skill_name = if let Some(n) = name {
        n
    } else {
        skill_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    println!(
        "{} {} -> {}",
        "➕".green(),
        skill_name.bold(),
        path.dimmed()
    );

    // Check if it's a valid skill (has SKILL.md)
    if !skill_path.join("SKILL.md").exists() {
        println!("{} SKILL.md not found in path", "⚠️ ".yellow());
        return Ok(());
    }

    // Move skill to workspace
    let workspace = &config.workspace;
    let dest = workspace.join(&skill_name);

    if dest.exists() {
        return Err(SkillsError::ConfigError(format!("Skill already exists in workspace: {}", skill_name)).into());
    }

    linker::move_skill(&skill_path, &dest).await?;

    // Create symlink at original location
    linker::create_symlink(&dest, &skill_path).await?;

    println!("{}", "✅ Skill added to workspace".green());
    println!("  Workspace: {}", dest.display());

    Ok(())
}

async fn cmd_remove(config: &Config, name: String) -> Result<()> {
    println!("{} {}", "🗑️ ".red(), name.bold());

    let workspace = &config.workspace;
    let skill_path = workspace.join(&name);

    if !skill_path.exists() {
        return Err(SkillsError::SkillNotFound(name).into());
    }

    // Remove the skill directory
    std::fs::remove_dir_all(&skill_path)?;

    println!("{}", "✅ Skill removed from workspace".green());

    Ok(())
}

async fn cmd_list(config: &Config, detailed: bool) -> Result<()> {
    let skills = config.list_skills();

    // Get configured tools
    let tools: Vec<_> = config.tools.values()
        .filter(|t| t.enabled)
        .collect();

    if skills.is_empty() {
        println!("{}", "No skills found in workspace.".dimmed());
        println!();
        println!("Use {} to add skills.", "skills add <name> <path>".cyan());
        return Ok(());
    }

    println!("{}", "📚 Skills in Workspace".bold());
    println!("{}", "════════════════════════".dimmed());
    println!();

    for skill in &skills {
        print!("{}", skill.name.bold());

        // Check which tools have this skill installed
        let mut installed_tools = Vec::new();
        for tool in &tools {
            let tool_path = tool.path.join(&skill.name);
            if tool_path.exists() {
                installed_tools.push(tool.name.clone());
            }
        }

        if !installed_tools.is_empty() {
            let tools_str = installed_tools.join(", ");
            print!(" [{}]", tools_str.cyan());
        } else {
            print!(" {}", "[not linked]".red().dimmed());
        }

        if detailed {
            println!();
            println!("  Path: {}", skill.path.display());
        } else {
            println!();
        }
    }

    println!();
    println!(
        "Total: {} {}",
        skills.len(),
        if skills.len() == 1 { "skill" } else { "skills" }
    );

    Ok(())
}

async fn cmd_sync(config: &Config, tool: Option<String>, dry_run: bool) -> Result<()> {
    println!("{}", "🔄 Syncing skills from tools...".cyan().bold());

    // Get scan paths based on tool selection
    let scan_paths = if let Some(tool_name) = tool {
        if let Some(tool_config) = config.tools.get(&tool_name) {
            vec![tool_config.path.clone()]
        } else {
            println!("{} Tool not found: {}", "⚠️ ".yellow(), tool_name);
            return Ok(());
        }
    } else {
        config.get_scan_paths()
    };

    let workspace = &config.workspace;
    let mut synced_count = 0;

    for scan_path in scan_paths {
        if !scan_path.exists() {
            continue;
        }

        println!("  Scanning: {}", scan_path.display());

        let skills = scanner::scan_directory(&scan_path, true).await?;

        for skill in skills {
            let dest = workspace.join(&skill.name);

            // Skip if already in workspace
            if dest.exists() {
                continue;
            }

            if dry_run {
                println!(
                    "  {} {} -> {}",
                    "[DRY RUN]".yellow(),
                    skill.name.bold(),
                    dest.display()
                );
            } else {
                println!("  {} {}", "→".cyan(), skill.name.bold());

                // Move skill to workspace
                linker::move_skill(&skill.path, &dest).await?;

                // Create symlink at original location
                linker::create_symlink(&dest, &skill.path).await?;

                synced_count += 1;
            }
        }
    }

    if !dry_run {
        println!();
        println!("{}", "✅ Sync complete!".green());
        println!("  Synced {} skills", synced_count);
    }

    Ok(())
}

async fn cmd_link(config: &Config, dry_run: bool) -> Result<()> {
    println!("{}", "🔗 Creating symlinks to tools...".cyan().bold());

    let skills = config.list_skills();
    let workspace = &config.workspace;

    if skills.is_empty() {
        println!("{} No skills found in workspace", "⚠️ ".yellow());
        return Ok(());
    }

    // Get enabled tools
    let tools: Vec<_> = config.tools.values()
        .filter(|t| t.enabled)
        .collect();

    if tools.is_empty() {
        println!("{} No enabled tools found", "⚠️ ".yellow());
        println!("  Enable tools in config first");
        return Ok(());
    }

    for skill in skills {
        let workspace_path = workspace.join(&skill.name);

        if !workspace_path.exists() {
            warn!("Skill not found in workspace: {}", skill.name);
            continue;
        }

        for tool in &tools {
            let tool_path = tool.path.join(&skill.name);

            if dry_run {
                println!(
                    "  {} {} -> {}",
                    "[DRY RUN]".yellow(),
                    workspace_path.display(),
                    tool_path.display()
                );
            } else {
                println!("  {} {} -> {}", "→".cyan(), skill.name.bold(), tool.name.cyan());
                linker::create_symlink(&workspace_path, &tool_path).await?;
            }
        }
    }

    if !dry_run {
        println!("{}", "✅ Links created!".green());
    }

    Ok(())
}

async fn cmd_unlink(config: &Config, name: String, dry_run: bool) -> Result<()> {
    println!("{} {}", "🔓 Unlinking", name.bold());

    let _skill = config.get_skill(&name)
        .ok_or_else(|| SkillsError::SkillNotFound(name.clone()))?;

    // Get enabled tools
    let tools: Vec<_> = config.tools.values()
        .filter(|t| t.enabled)
        .collect();

    let mut unlinked_count = 0;

    for tool in &tools {
        let tool_path = tool.path.join(&name);

        if tool_path.exists() && tool_path.is_symlink() {
            if dry_run {
                println!(
                    "  {} Would remove: {}",
                    "[DRY RUN]".yellow(),
                    tool_path.display()
                );
            } else {
                println!("  {} Removing from {}", "→".cyan(), tool.name.cyan());
                std::fs::remove_file(&tool_path)?;
                unlinked_count += 1;
            }
        }
    }

    if !dry_run {
        println!("{}", "✅ Unlinked!".green());
        println!("  Removed {} symlinks", unlinked_count);
    }

    Ok(())
}

async fn cmd_verify(config: &Config) -> Result<()> {
    println!("{}", "🔍 Verifying skills...".cyan().bold());

    let skills = config.list_skills();

    if skills.is_empty() {
        println!("{} No skills found in workspace", "⚠️ ".yellow());
        return Ok(());
    }

    // Get enabled tools
    let tools: Vec<_> = config.tools.values()
        .filter(|t| t.enabled)
        .collect();

    let mut all_valid = true;

    for skill in &skills {
        // Check if workspace copy exists
        if !skill.path.exists() {
            println!(
                "  {} {} (not found)",
                "✗".red(),
                skill.name.bold()
            );
            all_valid = false;
            continue;
        }

        // Check if linked to tools
        let mut linked_tools = Vec::new();
        for tool in &tools {
            let tool_path = tool.path.join(&skill.name);
            if tool_path.exists() {
                if tool_path.is_symlink() {
                    if let Ok(target) = std::fs::read_link(&tool_path) {
                        if target == skill.path {
                            linked_tools.push(tool.name.clone());
                        } else {
                            println!(
                                "  {} {} (wrong link target in {})",
                                "✗".red(),
                                skill.name.bold(),
                                tool.name.cyan()
                            );
                            all_valid = false;
                        }
                    }
                } else {
                    println!(
                        "  {} {} (not a symlink in {})",
                        "✗".red(),
                        skill.name.bold(),
                        tool.name.cyan()
                    );
                    all_valid = false;
                }
            }
        }

        if !linked_tools.is_empty() {
            println!("  {} {} [{}]", "✓".green(), skill.name.bold(), linked_tools.join(", ").cyan());
        } else {
            println!("  {} {} [not linked]", "○".yellow(), skill.name.bold());
        }
    }

    println!();
    if all_valid && !skills.is_empty() {
        println!("{}", "✅ All skills verified!".green());
    } else {
        println!("{}", "⚠️  Some skills have issues".yellow());
    }

    Ok(())
}

async fn cmd_config(show_path: bool) -> Result<()> {
    let config_path = Config::default_path();

    if show_path {
        println!("{}", config_path.display());
    } else {
        let config = Config::load(Some(config_path.to_str().unwrap()))?;
        println!("{}", "Current configuration:".bold());
        println!("{}", "═════════════════════════".dimmed());
        println!();
        println!("Workspace: {}", config.workspace.display());
        println!();

        if !config.tools.is_empty() {
            println!("{}", "Tools:".bold());
            for (name, tool) in &config.tools {
                println!("  {}:", name.cyan());
                println!("    Path: {}", tool.path.display());
                println!("    Enabled: {}", if tool.enabled { "✓" } else { "✗" });
            }
        } else {
            println!("{}", "No tools configured.".dimmed());
        }

        println!();
        println!("{}", "Skills:".bold());
        let skills = config.list_skills();
        if !skills.is_empty() {
            println!("  {} skills in workspace", skills.len());
        } else {
            println!("  No skills found in workspace");
        }
    }

    Ok(())
}

async fn cmd_tui() -> Result<()> {
    // Load config
    let config_path = Config::default_path();
    let mut config = Config::load(Some(config_path.to_str().unwrap())).unwrap_or_default();

    // Initialize terminal
    let mut terminal = tui::init_terminal()?;

    // Run TUI and get the state with user's selections
    let result = tui::run_tui(&mut terminal, &config)?;

    // Restore terminal
    tui::restore_terminal(&mut terminal)?;

    // Apply changes if user confirmed
    if let Some(state) = result {
        println!("{}", "Applying changes...".cyan().bold());
        tui::apply_selection(&mut config, &state).await?;
        println!("{}", "✅ Changes applied!".green());
    } else {
        println!("{}", "No changes applied.".yellow());
    }

    Ok(())
}

/// Extract repository name from GitHub URL
fn extract_repo_name_from_url(url: &str) -> Option<String> {
    // Parse URL and extract repo name
    // Supports formats like:
    // - https://github.com/user/repo
    // - https://github.com/user/repo.git
    // - git@github.com:user/repo.git

    if url.contains("github.com") {
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 2 {
            let repo = parts.last()?.trim_end_matches(".git");
            Some(repo.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

/// Clone a GitHub repository and return the path
fn clone_github_repo(url: &str) -> Result<std::path::PathBuf, SkillsError> {
    use std::process::Command;

    // Create a temporary directory for cloning
    let temp_dir = std::env::temp_dir().join(format!("skills-clone-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| SkillsError::ConfigError(format!("Failed to create temp dir: {}", e)))?;

    println!("{} Cloning repository to temporary directory...", "🔄".cyan());
    println!("  {}", temp_dir.display());

    // Use git command to clone
    let output = Command::new("git")
        .args(["clone", "--depth", "1", url, temp_dir.to_str().unwrap()])
        .output()
        .map_err(|e| SkillsError::ConfigError(format!("Failed to execute git clone: {}", e)))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(SkillsError::ConfigError(format!("Failed to clone repository: {}", error_msg)));
    }

    println!("{}", "✅ Repository cloned successfully".green());
    Ok(temp_dir)
}

/// Install skills from a GitHub repository
async fn cmd_install_from_github(
    config: &Config,
    url: &str,
    specific_skill: Option<String>,
    _tool: Option<String>,
    keep_repo: bool,
) -> Result<()> {
    use colored::Colorize;

    // Clone the repository
    let repo_path = clone_github_repo(url)?;

    // Scan for skills recursively
    println!("{}", "🔍 Scanning for skills in repository...".cyan().bold());
    let skills = scanner::scan_directory_sync(&repo_path, true)?;

    if skills.is_empty() {
        println!("{} No skills found in repository", "⚠️ ".yellow());
        // Clean up if not keeping
        if !keep_repo {
            std::fs::remove_dir_all(&repo_path)?;
        }
        return Ok(());
    }

    println!();
    println!("{}", "Found skills:".bold());
    for (i, skill) in skills.iter().enumerate() {
        println!(
            "  {}. {} ({})",
            (i + 1).to_string().cyan().bold(),
            skill.name.bold(),
            skill.path.display().to_string().dimmed()
        );
    }
    println!();

    // If specific skill is requested, install it directly
    if let Some(skill_name) = specific_skill {
        let skill = skills.iter().find(|s| s.name == skill_name);

        if let Some(skill) = skill {
            return install_single_skill(config, skill).await;
        } else {
            println!("{} Skill not found: {}", "⚠️ ".yellow(), skill_name);
            if !keep_repo {
                std::fs::remove_dir_all(&repo_path)?;
            }
            return Ok(());
        }
    }

    // Interactive selection
    println!("{}", "Select skills to install (comma-separated, e.g., 1,3,5 or 'all'):".bold());
    print!("> ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.eq_ignore_ascii_case("all") {
        // Install all skills
        println!();
        for skill in &skills {
            install_single_skill(config, skill).await?;
        }
    } else {
        // Parse selection
        let indices: Vec<usize> = input
            .split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect();

        println!();
        for i in indices {
            if i > 0 && i <= skills.len() {
                let skill = &skills[i - 1];
                install_single_skill(config, skill).await?;
            } else {
                println!("{} Invalid selection: {}", "⚠️ ".yellow(), i);
            }
        }
    }

    // Clean up if not keeping
    if !keep_repo {
        println!();
        println!("{}", "🧹 Cleaning up temporary files...".cyan());
        std::fs::remove_dir_all(&repo_path)?;
        println!("{}", "✅ Cleanup complete".green());
    }

    Ok(())
}

/// Install a single skill to the workspace
async fn install_single_skill(config: &Config, skill: &scanner::SkillInfo) -> Result<()> {
    use colored::Colorize;

    println!("{} {} -> workspace",
        "➕".green(),
        skill.name.bold()
    );

    let workspace = &config.workspace;
    let dest = workspace.join(&skill.name);

    if dest.exists() {
        println!("  {} Already installed, skipping...", "⊙".yellow());
        return Ok(());
    }

    // Copy skill to workspace (don't move since it's in a temp dir)
    linker::move_skill(&skill.path, &dest).await?;

    println!("  {} Installed successfully", "✓".green());

    Ok(())
}

/// Install a skill from ClawHub
async fn cmd_install_from_clawhub(
    config: &Config,
    slug: &str,
    specific_name: Option<String>,
    _keep_repo: bool,
) -> Result<()> {
    use colored::Colorize;

    println!("{} Fetching skill metadata...", "🔄".cyan());

    // Get token from config
    let token = config.clawhub.token.as_deref();
    let registry = Some(config.clawhub.registry.as_str());

    // Check if token is configured
    if token.is_none() {
        println!("{} ClawHub token not configured", "⚠️ ".yellow());
        println!();
        println!("To install skills from ClawHub, you need to configure your API token:");
        println!();
        println!("  1. Visit https://clawhub.ai");
        println!("  2. Go to Settings → API tokens");
        println!("  3. Create a new API token");
        println!("  4. Add the token to your config file:");
        println!();
        println!("     {}", Config::default_path().display().to_string().cyan());
        println!();
        println!("  Config file format:");
        println!("     {}", "clawhub:".bold());
        println!("       {} {}", "token:".bold(), "<your-api-token>");
        println!();
        println!("  Or edit with:");
        println!("     {}", "vim ~/.config/skills/config.yaml".cyan());
        return Err(SkillsError::ConfigError("ClawHub token not configured".to_string()).into());
    }

    // Get skill metadata
    let meta = clawhub::get_skill_meta(slug, registry, token).await?;

    // Check moderation status
    if let Some(moderation) = &meta.moderation {
        if moderation.is_malware_blocked {
            println!("{} This skill has been flagged as malicious and cannot be installed.", "⛔".red());
            return Err(SkillsError::ConfigError(format!("Skill {} is blocked as malware", slug)).into());
        }
        if moderation.is_suspicious {
            println!("{} Warning: This skill is flagged as suspicious.", "⚠️".yellow());
            println!("  It may contain risky patterns (crypto keys, external APIs, eval, etc.)");
            println!("  Review the skill code before use.");
            println!();
            print!("{} Install anyway? [y/N]: ", "?".bold());
            use std::io::Write;
            std::io::stdout().flush().unwrap();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("{} Installation cancelled", "⊙".yellow());
                return Ok(());
            }
        }
    }

    // Get version
    let version = meta.latest_version
        .as_ref()
        .map(|v| v.version.clone())
        .unwrap_or_else(|| "latest".to_string());

    println!("{} Downloading {}@{}...", "📦".cyan(), slug.bold(), version);

    // Download skill
    let zip_bytes = clawhub::download_skill(slug, &version, registry, token).await?;

    // Extract to temp directory
    let temp_dir = std::env::temp_dir().join(format!("skill-{}-{}", slug, std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| SkillsError::ConfigError(format!("Failed to create temp dir: {}", e)))?;

    println!("{} Extracting...", "📂".cyan());
    clawhub::extract_zip(&zip_bytes, &temp_dir)?;

    // Find SKILL.md
    let skill_name = if let Some(n) = specific_name {
        n
    } else {
        slug.to_string()
    };

    // Verify it's a valid skill
    if !temp_dir.join("SKILL.md").exists() {
        println!("{} SKILL.md not found in downloaded package", "⚠️ ".yellow());
        // Clean up
        std::fs::remove_dir_all(&temp_dir)?;
        return Ok(());
    }

    // Move to workspace
    let workspace = &config.workspace;
    let dest = workspace.join(&skill_name);

    if dest.exists() {
        println!("{} Skill already exists in workspace: {}", "⚠️ ".yellow(), skill_name);
        // Clean up
        std::fs::remove_dir_all(&temp_dir)?;
        return Ok(());
    }

    println!("{} {} -> workspace", "➕".green(), skill_name.bold());

    // Move skill to workspace
    linker::move_skill(&temp_dir, &dest).await?;

    // Clean up temp directory
    std::fs::remove_dir_all(&temp_dir)?;

    println!("{} Installed successfully from ClawHub", "✅".green());
    println!("  Slug: {}", slug.cyan());
    println!("  Version: {}", version.cyan());
    println!("  Workspace: {}", dest.display());

    Ok(())
}

