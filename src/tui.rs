use crate::config::Config;
use crate::linker;
use crate::scanner;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// TUI state
pub struct TuiState {
    /// Available skills grouped by tool
    pub skills_by_tool: HashMap<String, Vec<SkillItem>>,
    /// All unique skills across all tools
    pub all_skills: Vec<String>,
    /// All tools
    pub all_tools: Vec<String>,
    /// Installation status: skill_name -> tool_name -> installed
    pub installation_status: HashMap<String, HashMap<String, bool>>,
    /// Current selected tool index
    pub selected_tool: usize,
    /// Current selected skill index
    pub selected_skill: usize,
    /// List state for tools
    pub tool_list_state: ListState,
    /// List state for skills
    pub skill_list_state: ListState,
    /// Skills that are selected (enabled)
    pub selected_skills: HashMap<String, bool>,
    /// Current mode
    pub mode: Mode,
    /// Current view
    pub view: View,
    /// Status message
    pub status_message: String,
    /// Workspace path
    pub workspace_path: PathBuf,
    /// Original paths (for creating symlinks)
    pub original_paths: HashMap<String, PathBuf>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Help,
    Confirm,
}

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    ToolView,
    SkillView,
}

/// Skill item in the list
#[derive(Clone, Debug)]
pub struct SkillItem {
    pub name: String,
    pub tool: String,
    pub original_path: PathBuf,
    pub currently_enabled: bool,
}

impl TuiState {
    pub fn new(config: &Config) -> Result<Self> {
        let mut skills_by_tool: HashMap<String, Vec<SkillItem>> = HashMap::new();
        let mut selected_skills = HashMap::new();
        let mut original_paths = HashMap::new();
        let mut all_skills_set = std::collections::HashSet::new();
        let mut all_tools = Vec::new();
        let mut installation_status: HashMap<String, HashMap<String, bool>> = HashMap::new();

        // Collect enabled tools
        for (tool_name, tool_config) in &config.tools {
            if !tool_config.enabled {
                continue;
            }
            all_tools.push(tool_name.clone());
        }

        // Scan workspace for all skills (actual skill directories, not symlinks)
        let workspace_skills = if config.workspace.exists() {
            scanner::scan_directory_sync(&config.workspace, false).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Process each skill from workspace
        for skill_info in workspace_skills {
            let skill_name = skill_info.name.clone();
            all_skills_set.insert(skill_name.clone());
            original_paths.insert(skill_name.clone(), skill_info.path.clone());
            selected_skills.insert(skill_name.clone(), true);

            // Check which tools have this skill installed (by checking for symlinks)
            for tool_name in &all_tools {
                if let Some(tool_config) = config.tools.get(tool_name) {
                    let tool_skill_path = tool_config.path.join(&skill_name);

                    // Check if symlink exists and points to workspace
                    let is_installed = if tool_skill_path.is_symlink() {
                        if let Ok(target) = std::fs::read_link(&tool_skill_path) {
                            // Check if it points to the workspace skill
                            target == config.workspace.join(&skill_name)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    installation_status
                        .entry(skill_name.clone())
                        .or_insert_with(HashMap::new)
                        .insert(tool_name.clone(), is_installed);

                    // Add skill to tool's skill list (all skills, not just installed ones)
                    skills_by_tool
                        .entry(tool_name.clone())
                        .or_insert_with(Vec::new)
                        .push(SkillItem {
                            name: skill_name.clone(),
                            tool: tool_name.clone(),
                            original_path: skill_info.path.clone(),
                            currently_enabled: is_installed,
                        });
                }
            }
        }

        // Ensure all tools have entries in skills_by_tool
        for tool_name in &all_tools {
            skills_by_tool.entry(tool_name.clone()).or_insert_with(Vec::new);
        }

        // Sort skills and tools alphabetically
        let mut all_skills: Vec<_> = all_skills_set.into_iter().collect();
        all_skills.sort();
        all_tools.sort();

        // Sort skills in each tool's list
        for skills_list in skills_by_tool.values_mut() {
            skills_list.sort_by(|a, b| a.name.cmp(&b.name));
        }

        let mut tool_list_state = ListState::default();
        tool_list_state.select(Some(0));

        let mut skill_list_state = ListState::default();
        skill_list_state.select(Some(0));

        Ok(Self {
            skills_by_tool,
            all_skills,
            all_tools,
            installation_status,
            selected_tool: 0,
            selected_skill: 0,
            tool_list_state,
            skill_list_state,
            selected_skills,
            mode: Mode::Normal,
            view: View::ToolView,
            status_message: "Use arrow keys to navigate, Space to toggle, v to switch view, Enter to apply, q to quit".to_string(),
            workspace_path: config.workspace.clone(),
            original_paths,
        })
    }

    pub fn get_current_tool(&self) -> Option<&String> {
        self.all_tools.get(self.selected_tool)
    }

    pub fn get_current_skills(&self) -> Option<&Vec<SkillItem>> {
        self.get_current_tool()
            .and_then(|tool| self.skills_by_tool.get(tool))
    }

    pub fn get_current_skill(&self) -> Option<&SkillItem> {
        self.get_current_skills()
            .and_then(|skills| skills.get(self.selected_skill))
    }

    pub fn toggle_current_skill(&mut self) {
        if let Some(tool) = self.get_current_tool() {
            if let Some(skills) = self.get_current_skills() {
                if let Some(skill) = skills.get(self.selected_skill) {
                    let skill_name = skill.name.clone();
                    let tool_name = tool.clone();

                    // Update installation status
                    let status_map = self.installation_status
                        .entry(skill_name.clone())
                        .or_insert_with(HashMap::new);

                    let current_status = status_map.get(&tool_name).copied().unwrap_or(false);
                    status_map.insert(tool_name.clone(), !current_status);

                    // Update skill's currently_enabled flag in skills_by_tool
                    if let Some(tool_skills) = self.skills_by_tool.get_mut(&tool_name) {
                        if let Some(s) = tool_skills.iter_mut().find(|s| s.name == skill_name) {
                            s.currently_enabled = !current_status;
                        }
                    }

                    self.status_message = format!(
                        "{} {} for {}",
                        if !current_status { "Installed" } else { "Uninstalled" },
                        skill_name,
                        tool_name
                    );
                }
            }
        }
    }

    pub fn next_tool(&mut self) {
        if self.selected_tool < self.all_tools.len().saturating_sub(1) {
            self.selected_tool += 1;
            self.selected_skill = 0;
            self.skill_list_state.select(Some(0));
        }
    }

    pub fn previous_tool(&mut self) {
        if self.selected_tool > 0 {
            self.selected_tool -= 1;
            self.selected_skill = 0;
            self.skill_list_state.select(Some(0));
        }
    }

    pub fn next_skill(&mut self) {
        if let Some(skills) = self.get_current_skills() {
            if self.selected_skill < skills.len().saturating_sub(1) {
                self.selected_skill += 1;
            }
        }
    }

    pub fn previous_skill(&mut self) {
        if self.selected_skill > 0 {
            self.selected_skill -= 1;
        }
    }

    // Skill view methods
    pub fn get_current_skill_name(&self) -> Option<&String> {
        self.all_skills.get(self.selected_skill)
    }

    pub fn get_installation_status_for_current_skill(&self) -> HashMap<String, bool> {
        if let Some(skill_name) = self.get_current_skill_name() {
            self.installation_status.get(skill_name)
                .cloned()
                .unwrap_or_default()
        } else {
            HashMap::new()
        }
    }

    pub fn toggle_tool_for_current_skill(&mut self) {
        if let Some(skill_name) = self.get_current_skill_name().cloned() {
            if let Some(tool_name) = self.all_tools.get(self.selected_tool).cloned() {
                let status_map = self.installation_status
                    .entry(skill_name.clone())
                    .or_insert_with(HashMap::new);

                let current_status = status_map.get(&tool_name).copied().unwrap_or(false);
                status_map.insert(tool_name.clone(), !current_status);

                self.status_message = format!(
                    "{} {} for {}",
                    if !current_status { "Installed" } else { "Uninstalled" },
                    skill_name,
                    tool_name
                );
            }
        }
    }

    pub fn switch_view(&mut self) {
        self.view = match self.view {
            View::ToolView => View::SkillView,
            View::SkillView => View::ToolView,
        };
        self.selected_tool = 0;
        self.selected_skill = 0;
        self.status_message = format!(
            "Switched to {} view",
            if self.view == View::ToolView { "Tool" } else { "Skill" }
        );
    }
}

pub fn run_tui<B: Backend>(terminal: &mut Terminal<B>, config: &Config) -> Result<Option<TuiState>> {
    let mut state = TuiState::new(config)?;
    let mut should_apply = false;

    loop {
        terminal.draw(|f| ui(f, &mut state))?;

        // Wait for event with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match state.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('Q') => break,
                        KeyCode::Char('?') => state.mode = Mode::Help,
                        KeyCode::Char('v') | KeyCode::Char('V') => state.switch_view(),
                        KeyCode::Char(' ') => {
                            if state.view == View::ToolView {
                                state.toggle_current_skill();
                            } else {
                                state.toggle_tool_for_current_skill();
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => state.next_skill(),
                        KeyCode::Up | KeyCode::Char('k') => state.previous_skill(),
                        KeyCode::Tab => state.next_tool(),
                        KeyCode::BackTab => state.previous_tool(),
                        KeyCode::Left | KeyCode::Char('h') => state.previous_tool(),
                        KeyCode::Right | KeyCode::Char('l') => state.next_tool(),
                        KeyCode::Enter => {
                            state.mode = Mode::Confirm;
                            state.status_message = "Apply changes? (y=yes, n=no)".to_string();
                        }
                        _ => {}
                    },
                    Mode::Help => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => state.mode = Mode::Normal,
                        _ => {}
                    },
                    Mode::Confirm => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            should_apply = true;
                            break;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            state.mode = Mode::Normal;
                            state.status_message = "Cancelled".to_string();
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    Ok(if should_apply { Some(state) } else { None })
}

fn ui(f: &mut Frame, state: &mut TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),  // Header
                Constraint::Min(0),     // Main content
                Constraint::Length(3),  // Status
            ]
            .as_ref(),
        )
        .split(f.area());

    // Header
    let view_text = if state.view == View::ToolView {
        "Tool View"
    } else {
        "Skill View"
    };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Skills Manager", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - "),
            Span::styled(view_text, Style::default().fg(Color::Yellow)),
            Span::raw(" (Press v to switch)"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
    )
    .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // Main content
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(30),  // Tools list / Tools status
                Constraint::Percentage(70),  // Skills list / Skills list
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    if state.view == View::ToolView {
        // === TOOL VIEW ===
        // Tools list
        let tool_names: Vec<ListItem> = state
            .all_tools
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if i == state.selected_tool {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(name.as_str()).style(style)
            })
            .collect();

        let tools_list = List::new(tool_names)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Tools")
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut tool_list_state = state.tool_list_state.clone();
        tool_list_state.select(Some(state.selected_tool));
        f.render_stateful_widget(tools_list, main_chunks[0], &mut tool_list_state);

        // Skills list for current tool
        let skills_list_items: Vec<ListItem> = state
            .get_current_skills()
            .map(|skills| {
                skills
                    .iter()
                    .enumerate()
                    .map(|(i, skill)| {
                        let is_selected = i == state.selected_skill;
                        let is_enabled = skill.currently_enabled;

                        let prefix = if is_enabled { "[✓]" } else { "[ ]" };
                        let style = if is_selected {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else if !is_enabled {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default()
                        };

                        ListItem::new(format!("{} {}", prefix, skill.name)).style(style)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let skills_list = List::new(skills_list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Skills")
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut skill_list_state = state.skill_list_state.clone();
        skill_list_state.select(Some(state.selected_skill));
        f.render_stateful_widget(skills_list, main_chunks[1], &mut skill_list_state);
    } else {
        // === SKILL VIEW ===
        // Skills list
        let skills_list_items: Vec<ListItem> = state
            .all_skills
            .iter()
            .enumerate()
            .map(|(i, skill_name)| {
                let is_selected = i == state.selected_skill;

                let style = if is_selected {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(skill_name.as_str()).style(style)
            })
            .collect();

        let skills_list = List::new(skills_list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Skills")
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut skill_list_state = state.skill_list_state.clone();
        skill_list_state.select(Some(state.selected_skill));
        f.render_stateful_widget(skills_list, main_chunks[0], &mut skill_list_state);

        // Tools installation status for current skill
        let tool_status_items: Vec<ListItem> = state
            .all_tools
            .iter()
            .enumerate()
            .map(|(i, tool_name)| {
                let is_selected = i == state.selected_tool;
                let is_installed = state
                    .get_installation_status_for_current_skill()
                    .get(tool_name)
                    .copied()
                    .unwrap_or(false);

                let prefix = if is_installed { "[✓]" } else { "[ ]" };
                let style = if is_selected {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if !is_installed {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };

                ListItem::new(format!("{} {}", prefix, tool_name)).style(style)
            })
            .collect();

        let tools_status = List::new(tool_status_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title("Installation Status")
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut tool_list_state = state.tool_list_state.clone();
        tool_list_state.select(Some(state.selected_tool));
        f.render_stateful_widget(tools_status, main_chunks[1], &mut tool_list_state);
    }

    // Status bar
    let status = Paragraph::new(state.status_message.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(status, chunks[2]);

    // Help overlay
    if state.mode == Mode::Help {
        let help_area = centered_rect(60, 50, f.area());
        let help_text = vec![
            Line::from(vec![
                Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/k     Move up"),
            Line::from("  ↓/j     Move down"),
            Line::from("  ←/h     Previous tool"),
            Line::from("  →/l     Next tool"),
            Line::from("  Tab     Next tool"),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  Space   Toggle skill/installation"),
            Line::from("  v       Switch view (Tool/Skill)"),
            Line::from("  Enter   Apply changes"),
            Line::from("  ?       Show help"),
            Line::from("  q       Quit"),
            Line::from(""),
            Line::from("Views:"),
            Line::from("  Tool View   - Browse by tool, select skills"),
            Line::from("  Skill View  - Browse by skill, install to tools"),
        ];

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(Color::DarkGray))
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(help, help_area);
    }

    // Confirm dialog
    if state.mode == Mode::Confirm {
        let confirm_area = centered_rect(50, 20, f.area());
        let confirm_text = vec![
            Line::from(vec![
                Span::styled("Apply Changes?", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from("This will:"),
            Line::from("  1. Move selected skills to workspace"),
            Line::from("  2. Create symlinks at original locations"),
            Line::from(""),
            Line::from("Press y to confirm, n to cancel"),
        ];

        let confirm = Paragraph::new(confirm_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .style(Style::default().bg(Color::DarkGray))
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(confirm, confirm_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

/// Apply selected skills configuration
pub async fn apply_selection(config: &mut Config, state: &TuiState) -> Result<()> {
    // Get workspace path
    let workspace = &config.workspace;

    // Process all skills
    for skill_name in &state.all_skills {
        // The skill should already be in workspace
        let workspace_skill_path = workspace.join(skill_name);

        // If skill is not in workspace, try to find and move it
        if !workspace_skill_path.exists() {
            if let Some(original_path) = state.original_paths.get(skill_name) {
                // Only move if it's a real directory (not a symlink)
                if original_path.exists() && !original_path.is_symlink() {
                    linker::move_skill(original_path, &workspace_skill_path).await?;
                }
            }
        }

        // Install to each tool based on installation status
        let empty_map = HashMap::new();
        let install_status = state.installation_status.get(skill_name).unwrap_or(&empty_map);

        for (tool_name, &is_installed) in install_status {
            if let Some(tool_config) = config.tools.get(tool_name) {
                let tool_skill_path = tool_config.path.join(skill_name);

                if is_installed {
                    // Ensure tool directory exists
                    if let Some(parent) = tool_skill_path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }

                    // Remove existing file/symlink at tool location
                    if tool_skill_path.exists() {
                        if tool_skill_path.is_symlink() || tool_skill_path.is_file() {
                            std::fs::remove_file(&tool_skill_path)?;
                        } else if tool_skill_path.is_dir() {
                            std::fs::remove_dir_all(&tool_skill_path)?;
                        }
                    }

                    // Create symlink from tool directory to workspace
                    // tool_skill_path -> workspace_skill_path
                    linker::create_symlink(&workspace_skill_path, &tool_skill_path).await?;
                } else {
                    // Remove symlink if exists
                    if tool_skill_path.is_symlink() {
                        std::fs::remove_file(&tool_skill_path)?;
                    }
                }
            }
        }
    }

    // Save config (without skills - they're dynamically scanned)
    config.save(&Config::default_path())?;

    Ok(())
}

pub fn init_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
