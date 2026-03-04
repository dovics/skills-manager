mod cli;
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
        Commands::Add { name, path, tool } => {
            cmd_add(&config, name, path, tool).await?;
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

async fn cmd_add(config: &Config, name: String, path: String, tool: Option<String>) -> Result<()> {
    println!(
        "{} {} -> {}",
        "➕".green(),
        name.bold(),
        path.dimmed()
    );

    let skill_path = std::path::PathBuf::from(&path);

    if !skill_path.exists() {
        return Err(SkillsError::PathNotFound(path).into());
    }

    // Add to config
    let mut config = config.clone();
    config.add_skill(&name, &skill_path, tool.as_deref())?;
    config.save(Config::default_path().as_path())?;

    println!("{}", "✅ Skill added to config".green());

    Ok(())
}

async fn cmd_remove(config: &Config, name: String) -> Result<()> {
    println!("{} {}", "🗑️ ".red(), name.bold());

    let mut config = config.clone();
    config.remove_skill(&name)?;
    config.save(Config::default_path().as_path())?;

    println!("{}", "✅ Skill removed from config".green());

    Ok(())
}

async fn cmd_list(config: &Config, detailed: bool) -> Result<()> {
    // Discover tools (including ones not in config)
    let mut discovered_tools = std::collections::HashMap::new();

    // Common tool paths to check
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let common_paths = [
        ("claude-code", format!("{}/.claude/skills", home)),
        ("openclaw", format!("{}/.openclaw/skills", home)),
        ("zeroclaw", format!("{}/.zeroclaw/skills", home)),
        ("npx-skills", format!("{}/.agents/skills", home)),
    ];

    // Check which tools exist on the system
    for (name, path_str) in common_paths {
        let path = std::path::PathBuf::from(&path_str);
        let exists = path.exists();
        let skill_count = if exists {
            scanner::scan_directory_sync(&path, false).unwrap_or_default().len()
        } else {
            0
        };

        discovered_tools.insert(
            name.to_string(),
            (path, exists, skill_count),
        );
    }

    // Display tools section
    println!("{}", "🔧 AI Tools Status".bold());
    println!("{}", "═════════════════".dimmed());
    println!();

    // Display all known tools (both configured and discovered)
    let mut all_tool_names: std::collections::HashSet<String> =
        config.tools.keys().cloned().collect();
    for name in discovered_tools.keys() {
        all_tool_names.insert(name.clone());
    }

    let mut tool_names: Vec<_> = all_tool_names.into_iter().collect();
    tool_names.sort();

    for tool_name in tool_names {
        let is_configured = config.tools.contains_key(&tool_name);

        // Get path and status
        let (path, exists, skill_count) = discovered_tools
            .get(&tool_name)
            .map(|(p, e, c)| (p.clone(), *e, *c))
            .unwrap_or_else(|| {
                // Use configured path if not in common paths
                let tool_config = config.tools.get(&tool_name);
                let path = tool_config.map(|t| t.path.clone())
                    .unwrap_or_else(|| std::path::PathBuf::from("<unknown>"));
                let exists = path.exists();
                let skill_count = if exists {
                    scanner::scan_directory_sync(&path, false).unwrap_or_default().len()
                } else {
                    0
                };
                (path, exists, skill_count)
            });

        let is_enabled = config.tools.get(&tool_name).map(|t| t.enabled).unwrap_or(false);

        // Tool status indicator
        let status_indicator = if exists {
            "✓".green()
        } else {
            "✗".red()
        };

        // Tool name and status
        let configured_mark = if is_configured { "●" } else { "○" };
        let enabled_mark = if is_enabled { "✓" } else { "✗" };
        let tool_display = format!("{} {} [{}]", configured_mark, tool_name.cyan().bold(), enabled_mark);

        println!("  {} {} - {}",
                 status_indicator,
                 tool_display,
                 if exists {
                     format!("{} skills", skill_count).green()
                 } else {
                     "not installed".red().dimmed()
                 });

        if detailed {
            println!("    Path: {}", path.display());
        }
    }

    println!();

    // Display managed skills section
    let skills = config.list_skills();

    if !skills.is_empty() {
        println!("{}", "📚 Managed Skills".bold());
        println!("{}", "═════════════════".dimmed());
        println!();

        for skill in &skills {
            if detailed {
                println!("{}", skill.name.bold());
                println!("  Path: {}", skill.path.display());
                if let Some(tool) = &skill.tool {
                    println!("  Tool: {}", tool);
                }
                println!();
            } else {
                let tool_label = skill.tool.as_ref()
                    .map(|t| format!(" ({})", t.cyan()))
                    .unwrap_or_default();
                println!("  • {}{}", skill.name.bold(), tool_label);
            }
        }

        println!();
        println!(
            "Total: {} {}",
            skills.len(),
            if skills.len() == 1 { "skill" } else { "skills" }
        );
        println!();
    } else {
        println!("{}", "No skills managed yet.".dimmed());
        println!();
        println!("Use {} to add skills.", "skills add <name> <path>".cyan());
        println!();
    }

    // Show help for adding tools
    println!("{}", "Legend:".bold());
    println!("  ● = Configured  ○ = Not configured");
    println!("  ✓ = Installed   ✗ = Not installed");
    println!("  [✓] = Enabled    [✗] = Disabled");
    println!();
    println!("Add missing tools with: {}", "skills config".cyan());

    Ok(())
}

async fn cmd_sync(config: &Config, tool: Option<String>, dry_run: bool) -> Result<()> {
    println!("{}", "🔄 Syncing skills...".cyan().bold());

    let skills_to_sync = if let Some(tool_name) = tool {
        config.get_skills_by_tool(&tool_name)
    } else {
        config.list_skills()
    };

    let workspace = Config::default_workspace_path();

    for skill in skills_to_sync {
        let dest = workspace.join(&skill.name);

        if dry_run {
            println!(
                "  {} {} -> {}",
                "[DRY RUN]".yellow(),
                skill.path.display(),
                dest.display()
            );
        } else {
            println!("  {} {}", "→".cyan(), skill.name.bold());

            // Move skill to workspace
            linker::move_skill(&skill.path, &dest).await?;

            // Create symlink at original location
            linker::create_symlink(&dest, &skill.path).await?;

            // Update config with new path
            // (config.update_skill_path(&skill.name, &dest)?);
        }
    }

    if !dry_run {
        println!("{}", "✅ Sync complete!".green());
    }

    Ok(())
}

async fn cmd_link(config: &Config, dry_run: bool) -> Result<()> {
    println!("{}", "🔗 Creating symlinks...".cyan().bold());

    let skills = config.list_skills();
    let workspace = Config::default_workspace_path();

    for skill in skills {
        let workspace_path = workspace.join(&skill.name);

        if !workspace_path.exists() {
            warn!("Skill not found in workspace: {}", skill.name);
            continue;
        }

        if dry_run {
            println!(
                "  {} {} -> {}",
                "[DRY RUN]".yellow(),
                workspace_path.display(),
                skill.path.display()
            );
        } else {
            println!("  {} {}", "→".cyan(), skill.name.bold());
            linker::create_symlink(&workspace_path, &skill.path).await?;
        }
    }

    if !dry_run {
        println!("{}", "✅ Links created!".green());
    }

    Ok(())
}

async fn cmd_unlink(config: &Config, name: String, dry_run: bool) -> Result<()> {
    println!("{} {}", "🔓 Unlinking", name.bold());

    let skill = config.get_skill(&name)?;
    let workspace = Config::default_workspace_path();
    let workspace_path = workspace.join(&name);

    if dry_run {
        println!(
            "  {} Would remove symlink: {}",
            "[DRY RUN]".yellow(),
            skill.path.display()
        );
        println!(
            "  {} Would restore to: {}",
            "[DRY RUN]".yellow(),
            workspace_path.display()
        );
    } else {
        // Remove symlink
        if skill.path.exists() && skill.path.is_symlink() {
            std::fs::remove_file(&skill.path)?;
        }

        // Move back from workspace
        if workspace_path.exists() {
            std::fs::rename(&workspace_path, &skill.path)?;
        }

        println!("{}", "✅ Unlinked!".green());
    }

    Ok(())
}

async fn cmd_verify(config: &Config) -> Result<()> {
    println!("{}", "🔍 Verifying skills...".cyan().bold());

    let skills = config.list_skills();
    let workspace = Config::default_workspace_path();

    let mut all_valid = true;

    for skill in skills {
        let workspace_path = workspace.join(&skill.name);

        // Check if workspace copy exists
        if !workspace_path.exists() {
            println!(
                "  {} {} (not in workspace)",
                "✗".red(),
                skill.name.bold()
            );
            all_valid = false;
            continue;
        }

        // Check if symlink exists and is valid
        if skill.path.is_symlink() {
            let target = std::fs::read_link(&skill.path)?;
            if target == workspace_path {
                println!("  {} {}", "✓".green(), skill.name.bold());
            } else {
                println!(
                    "  {} {} (wrong link target)",
                    "✗".red(),
                    skill.name.bold()
                );
                all_valid = false;
            }
        } else {
            println!("  {} {} (not linked)", "○".yellow(), skill.name.bold());
        }
    }

    println!();
    if all_valid {
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
            println!();
        }

        if !config.skills.is_empty() {
            println!("{}", "Skills:".bold());
            for (name, skill) in &config.skills {
                println!("  {}: {}", name.cyan(), skill.path.display());
            }
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

    // Run TUI
    let should_apply = tui::run_tui(&mut terminal, &config)?;

    // Restore terminal
    tui::restore_terminal(&mut terminal)?;

    // Apply changes if user confirmed
    if should_apply {
        println!("{}", "Applying changes...".cyan().bold());

        // Create a new state to apply the selection
        let state = tui::TuiState::new(&config)?;
        tui::apply_selection(&mut config, &state).await?;

        println!("{}", "✅ Changes applied!".green());
    } else {
        println!("{}", "No changes applied.".yellow());
    }

    Ok(())
}
