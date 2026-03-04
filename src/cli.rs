use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "skills")]
#[command(about = "A unified skills management tool for AI assistants", long_about = None)]
#[command(version = "0.1.0")]
#[command(author = "dovics")]
pub struct SkillsArgs {
    /// Path to config file (default: ~/.config/skills/config.yaml)
    #[arg(short, long)]
    pub config: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Initialize the skills manager
    Init {
        /// Force reinitialization (overwrites existing config)
        #[arg(short, long)]
        force: bool,
    },

    /// Scan directories for skills
    Scan {
        /// Path to scan (if not specified, uses configured paths)
        #[arg(short, long)]
        path: Option<String>,

        /// Scan recursively
        #[arg(short, long)]
        recursive: bool,
    },

    /// Add a skill to the workspace
    Add {
        /// Name of the skill
        name: String,

        /// Path to the skill directory
        path: String,

        /// Tool name (claude-code, openclaw, etc.)
        #[arg(short, long)]
        tool: Option<String>,
    },

    /// Remove a skill from management
    Remove {
        /// Name of the skill to remove
        name: String,
    },

    /// List all managed skills
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Sync skills to workspace and create symlinks
    Sync {
        /// Only sync skills for specific tool
        #[arg(short, long)]
        tool: Option<String>,

        /// Show what would be done without making changes
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Create symlinks for all skills
    Link {
        /// Show what would be done without making changes
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Unlink a skill (remove symlink, restore to original location)
    Unlink {
        /// Name of the skill to unlink
        name: String,

        /// Show what would be done without making changes
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Verify all skills and symlinks
    Verify,

    /// Show or edit configuration
    Config {
        /// Show only the config file path
        #[arg(short, long)]
        show_path: bool,
    },

    /// Launch interactive TUI for skill management
    Tui,
}
