use crate::config::{Config, SkillConfig};
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
        let mut skills_by_tool = HashMap::new();
        let mut selected_skills = HashMap::new();
        let mut original_paths = HashMap::new();

        // Scan all tools for skills
        for (tool_name, tool_config) in &config.tools {
            if !tool_config.enabled {
                continue;
            }

            // Use synchronous scan
            let skill_infos = scanner::scan_directory_sync(&tool_config.path, false);

            if let Ok(infos) = skill_infos {
                let items: Vec<SkillItem> = infos
                    .into_iter()
                    .map(|info| {
                        let skill_name = info.name.clone();
                        original_paths.insert(skill_name.clone(), info.path.clone());
                        selected_skills.insert(skill_name.clone(), true);

                        SkillItem {
                            name: info.name,
                            tool: tool_name.clone(),
                            original_path: info.path,
                            currently_enabled: true,
                        }
                    })
                    .collect();

                if !items.is_empty() {
                    skills_by_tool.insert(tool_name.clone(), items);
                }
            }
        }

        let mut tool_list_state = ListState::default();
        tool_list_state.select(Some(0));

        let mut skill_list_state = ListState::default();
        skill_list_state.select(Some(0));

        Ok(Self {
            skills_by_tool,
            selected_tool: 0,
            selected_skill: 0,
            tool_list_state,
            skill_list_state,
            selected_skills,
            mode: Mode::Normal,
            status_message: "Use arrow keys to navigate, Space to toggle, Enter to apply, q to quit".to_string(),
            workspace_path: config.workspace.clone(),
            original_paths,
        })
    }

    pub fn get_current_tool(&self) -> Option<&String> {
        self.skills_by_tool
            .keys()
            .nth(self.selected_tool)
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
        if let Some(skill) = self.get_current_skill() {
            let skill_name = skill.name.clone();
            let enabled = self.selected_skills.entry(skill_name.clone()).or_insert(true);
            *enabled = !*enabled;
            self.status_message = format!(
                "{} skill {}",
                if *enabled { "Enabled" } else { "Disabled" },
                skill_name
            );
        }
    }

    pub fn next_tool(&mut self) {
        if self.selected_tool < self.skills_by_tool.len().saturating_sub(1) {
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
}

pub fn run_tui<B: Backend>(terminal: &mut Terminal<B>, config: &Config) -> Result<bool> {
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
                        KeyCode::Char(' ') => state.toggle_current_skill(),
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

    Ok(should_apply)
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
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Skills Manager", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - "),
            Span::styled("Interactive Mode", Style::default().fg(Color::White)),
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
                Constraint::Percentage(30),  // Tools list
                Constraint::Percentage(70),  // Skills list
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    // Tools list
    let tool_names: Vec<ListItem> = state
        .skills_by_tool
        .keys()
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

    // Skills list
    let skills_list_items: Vec<ListItem> = state
        .get_current_skills()
        .map(|skills| {
            skills
                .iter()
                .enumerate()
                .map(|(i, skill)| {
                    let is_selected = i == state.selected_skill;
                    let is_enabled = state.selected_skills.get(&skill.name).copied().unwrap_or(true);

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
            Line::from("  Space   Toggle skill"),
            Line::from("  Enter   Apply changes"),
            Line::from("  ?       Show help"),
            Line::from("  q       Quit"),
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
    // Clear existing skills
    config.skills.clear();

    // Add selected skills
    for (skill_name, &enabled) in &state.selected_skills {
        if enabled {
            if let Some(original_path) = state.original_paths.get(skill_name) {
                // Move skill to workspace
                let dest = config.workspace.join(skill_name);
                linker::move_skill(original_path, &dest).await?;

                // Create symlink at original location
                linker::create_symlink(&dest, original_path).await?;

                // Add to config
                let tool_name = state
                    .skills_by_tool
                    .iter()
                    .find(|(_, skills)| skills.iter().any(|s| &s.name == skill_name))
                    .map(|(name, _)| name.clone());

                config.skills.insert(
                    skill_name.clone(),
                    SkillConfig {
                        name: skill_name.clone(),
                        path: original_path.clone(),
                        tool: tool_name,
                    },
                );
            }
        }
    }

    // Save config
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
