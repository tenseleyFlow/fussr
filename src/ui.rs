use crate::app::App;
use crate::types::{AppMode, CommitStatus, InputMode, PullStatus, PushStatus, SelectableItem};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Draw the complete UI
pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Header (repo:branch)
            Constraint::Min(3),    // Tree view
            Constraint::Length(3), // Help/status
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_tree(frame, app, chunks[1]);
    draw_help(frame, app, chunks[2]);

    // Draw modal overlay if in commit mode
    if let InputMode::Commit { buffer, cursor, amend, status } = &app.input_mode {
        draw_commit_modal(frame, buffer, *cursor, *amend, status);
    }

    // Draw modal overlay if in push mode
    if let InputMode::Push { remotes, selected, status } = &app.input_mode {
        draw_push_modal(frame, remotes, *selected, status, &app.branch_name);
    }

    // Draw modal overlay if in pull mode
    if let InputMode::Pull { remotes, selected, status } = &app.input_mode {
        draw_pull_modal(frame, remotes, *selected, status, &app.branch_name);
    }

    // Draw modal overlay if in confirm mode
    if let InputMode::Confirm { message, .. } = &app.input_mode {
        draw_confirm_modal(frame, message);
    }
}

/// Draw commit message modal
fn draw_commit_modal(frame: &mut Frame, buffer: &str, cursor: usize, amend: bool, status: &CommitStatus) {
    let area = frame.area();

    // Center the modal
    let modal_width = 60.min(area.width.saturating_sub(4));
    let modal_height = 5;
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    // Clear area behind modal
    frame.render_widget(Clear, modal_area);

    // Title and border color based on status
    let (title, border_color) = match status {
        CommitStatus::Editing => {
            let t = if amend { " Amend Commit " } else { " Commit " };
            (t, Color::Green)
        }
        CommitStatus::Committing => (" Committing... ", Color::Yellow),
        CommitStatus::Success => (" ✓ Committed ", Color::Green),
        CommitStatus::Failed => (" ✗ Failed ", Color::Red),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Content based on status
    let content = match status {
        CommitStatus::Editing => {
            // Build input line with cursor
            let display_text = if cursor >= buffer.len() {
                format!("{}█", buffer)
            } else {
                format!("{}█{}", &buffer[..cursor], &buffer[cursor..])
            };
            vec![
                Line::from(""),
                Line::from(Span::raw(display_text)),
            ]
        }
        CommitStatus::Committing => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Committing changes...",
                    Style::default().fg(Color::Yellow),
                )),
            ]
        }
        CommitStatus::Success => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ✓ Changes committed successfully!",
                    Style::default().fg(Color::Green),
                )),
            ]
        }
        CommitStatus::Failed => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ✗ Commit failed (nothing staged?)",
                    Style::default().fg(Color::Red),
                )),
            ]
        }
    };

    let input = Paragraph::new(content).block(block);
    frame.render_widget(input, modal_area);
}

/// Draw push remote selection modal
fn draw_push_modal(frame: &mut Frame, remotes: &[String], selected: usize, status: &PushStatus, branch: &str) {
    let area = frame.area();

    // Modal size based on content
    let modal_height = match status {
        PushStatus::SelectRemote => (remotes.len() + 4).min(12) as u16,
        _ => 5,
    };
    let modal_width = 50.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    // Clear area behind modal
    frame.render_widget(Clear, modal_area);

    // Title and border color based on status
    let (title, border_color) = match status {
        PushStatus::SelectRemote => (" Push - Select Remote ", Color::Cyan),
        PushStatus::Pushing => (" Pushing... ", Color::Yellow),
        PushStatus::Success => (" ✓ Pushed ", Color::Green),
        PushStatus::Failed(_) => (" ✗ Push Failed ", Color::Red),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Content based on status
    let content: Vec<Line> = match status {
        PushStatus::SelectRemote => {
            let mut lines = vec![
                Line::from(Span::styled(
                    format!("  Set upstream for '{}':", branch),
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
            ];
            for (i, remote) in remotes.iter().enumerate() {
                let style = if i == selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(format!("    {}", remote), style)));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  j/k:select Enter:push ESC:cancel",
                Style::default().fg(Color::DarkGray),
            )));
            lines
        }
        PushStatus::Pushing => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Pushing changes...",
                    Style::default().fg(Color::Yellow),
                )),
            ]
        }
        PushStatus::Success => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ✓ Changes pushed successfully!",
                    Style::default().fg(Color::Green),
                )),
            ]
        }
        PushStatus::Failed(msg) => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  ✗ {}", msg),
                    Style::default().fg(Color::Red),
                )),
            ]
        }
    };

    let widget = Paragraph::new(content).block(block);
    frame.render_widget(widget, modal_area);
}

/// Draw pull remote selection modal
fn draw_pull_modal(frame: &mut Frame, remotes: &[String], selected: usize, status: &PullStatus, branch: &str) {
    let area = frame.area();

    let modal_height = match status {
        PullStatus::SelectRemote => (remotes.len() + 4).min(12) as u16,
        _ => 5,
    };
    let modal_width = 50.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let (title, border_color) = match status {
        PullStatus::SelectRemote => (" Pull - Select Remote ", Color::Cyan),
        PullStatus::Pulling => (" Pulling... ", Color::Yellow),
        PullStatus::Success => (" ✓ Pulled ", Color::Green),
        PullStatus::Failed(_) => (" ✗ Pull Failed ", Color::Red),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let content: Vec<Line> = match status {
        PullStatus::SelectRemote => {
            let mut lines = vec![
                Line::from(Span::styled(
                    format!("  Set upstream for '{}':", branch),
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
            ];
            for (i, remote) in remotes.iter().enumerate() {
                let style = if i == selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(format!("    {}", remote), style)));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  j/k:select Enter:pull ESC:cancel",
                Style::default().fg(Color::DarkGray),
            )));
            lines
        }
        PullStatus::Pulling => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Pulling changes...",
                    Style::default().fg(Color::Yellow),
                )),
            ]
        }
        PullStatus::Success => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ✓ Changes pulled successfully!",
                    Style::default().fg(Color::Green),
                )),
            ]
        }
        PullStatus::Failed(msg) => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  ✗ {}", msg),
                    Style::default().fg(Color::Red),
                )),
            ]
        }
    };

    let widget = Paragraph::new(content).block(block);
    frame.render_widget(widget, modal_area);
}

/// Draw confirmation modal
fn draw_confirm_modal(frame: &mut Frame, message: &str) {
    let area = frame.area();

    let modal_width = 50.min(area.width.saturating_sub(4));
    let modal_height = 5;
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    // Clear area behind modal
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", message),
            Style::default().fg(Color::White),
        )),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("es / "),
            Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("o"),
        ]),
    ];

    let widget = Paragraph::new(content).block(block);
    frame.render_widget(widget, modal_area);
}

/// Draw header with repo:branch info
fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(&app.repo_name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(":"),
        Span::styled(&app.branch_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ];

    // Show mode indicator
    if app.mode == AppMode::Git {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "[ GIT MODE ]",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    // Show search buffer if active
    if !app.search_buffer.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("/{}", &app.search_buffer),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    }

    // Show status message if any
    if let Some(ref msg) = app.status_message {
        spans.push(Span::raw(" - "));
        spans.push(Span::styled(msg, Style::default().fg(Color::Green)));
    }

    let header = Paragraph::new(Line::from(spans));
    frame.render_widget(header, area);
}

/// Draw the file tree
fn draw_tree(frame: &mut Frame, app: &App, area: Rect) {
    let visible_height = area.height as usize;
    let total_items = app.items.len();

    // Calculate viewport bounds centered on selection
    let half_visible = visible_height / 2;
    let viewport_start = if app.selected > half_visible {
        (app.selected - half_visible).min(total_items.saturating_sub(visible_height))
    } else {
        0
    };
    let viewport_end = (viewport_start + visible_height).min(total_items);

    // Build tree lines
    let mut lines: Vec<Line> = Vec::new();

    // Add root "." on first line if viewport starts at 0
    if viewport_start == 0 && !app.items.is_empty() {
        lines.push(Line::from(Span::raw(".")));
    }

    // Add visible items
    for (i, item) in app.items.iter().enumerate().skip(viewport_start).take(viewport_end - viewport_start) {
        let line = render_tree_item(item, i == app.selected, &app.input_mode);
        lines.push(line);
    }

    let tree = Paragraph::new(lines).block(Block::default());
    frame.render_widget(tree, area);
}

/// Render a single tree item
fn render_tree_item(item: &SelectableItem, is_selected: bool, input_mode: &InputMode) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();

    // Build prefix based on depth and tree structure
    for (depth, &is_last) in item.ancestors_are_last.iter().enumerate() {
        if depth < item.ancestors_are_last.len() {
            if is_last {
                spans.push(Span::raw("    "));
            } else {
                spans.push(Span::raw("│   "));
            }
        }
    }

    // Add branch character
    if item.is_last_sibling {
        spans.push(Span::raw("└── "));
    } else {
        spans.push(Span::raw("├── "));
    }

    // Add expand/collapse indicator for directories
    if !item.is_file {
        if item.is_expanded {
            spans.push(Span::raw("▼ "));
        } else {
            spans.push(Span::raw("▶ "));
        }
    }

    // Handle rename mode
    if is_selected {
        if let InputMode::Rename { buffer, cursor } = input_mode {
            // Show editable name with cursor
            let name_with_cursor = if *cursor >= buffer.len() {
                format!("{}█", buffer)
            } else {
                format!("{}█{}", &buffer[..*cursor], &buffer[*cursor..])
            };
            spans.push(Span::styled(
                name_with_cursor,
                Style::default().add_modifier(Modifier::REVERSED),
            ));
        } else {
            // Normal selection highlight
            let name_style = if item.status.is_gitignored {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().add_modifier(Modifier::REVERSED)
            };
            spans.push(Span::styled(item.name.clone(), name_style));
        }
    } else {
        // Unselected item
        let name_style = if item.status.is_gitignored {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };
        spans.push(Span::styled(item.name.clone(), name_style));
    }

    // Add status indicators
    if item.status.is_staged {
        spans.push(Span::styled(" ↑", Style::default().fg(Color::Green)));
    }
    if item.status.is_unstaged {
        spans.push(Span::styled(" ✗", Style::default().fg(Color::Red)));
    }
    if item.status.is_untracked {
        spans.push(Span::styled(" ✗", Style::default().fg(Color::DarkGray)));
    }
    if item.status.has_incoming {
        spans.push(Span::styled(" ↓", Style::default().fg(Color::Blue)));
    }

    Line::from(spans)
}

/// Draw help/status bar
fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let (line1, line2) = match &app.input_mode {
        InputMode::Rename { .. } => (
            Line::from(vec![
                Span::styled(
                    "RENAME: ",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw("Type new name | Tab:save | ESC:cancel"),
            ]),
            Line::from(""),
        ),
        InputMode::Commit { .. } => (
            Line::from(vec![
                Span::styled(
                    "COMMIT: ",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                Span::raw("Type message | Enter:commit | ESC:cancel"),
            ]),
            Line::from(""),
        ),
        InputMode::Search { buffer } => (
            Line::from(vec![
                Span::styled("SEARCH: ", Style::default().fg(Color::Cyan)),
                Span::raw(buffer.clone()),
            ]),
            Line::from(""),
        ),
        _ => {
            let legend = Line::from(vec![
                Span::styled("↑", Style::default().fg(Color::Green)),
                Span::raw("=staged "),
                Span::styled("✗", Style::default().fg(Color::Red)),
                Span::raw("=mod "),
                Span::styled("✗", Style::default().fg(Color::DarkGray)),
                Span::raw("=new "),
                Span::styled("↓", Style::default().fg(Color::Blue)),
                Span::raw("=incoming"),
            ]);

            let keys = if app.mode == AppMode::Git {
                Line::from(vec![
                    Span::styled("GIT: ", Style::default().fg(Color::Yellow)),
                    Span::raw("a/u:stage S/U:all x:discard m:commit f:fetch l:pull p:push q/ESC:exit ^Q:quit"),
                ])
            } else {
                Line::from("j/k:nav ←/→:tree space:toggle .:dots alt-g:git ^Q:quit")
            };

            (legend, keys)
        }
    };

    let help = Paragraph::new(vec![line1, line2]);
    frame.render_widget(help, area);
}

/// Print tree to stdout (non-interactive mode)
pub fn print_tree(app: &App) {
    println!(".");
    for item in &app.items {
        print_tree_item(item);
    }
}

fn print_tree_item(item: &SelectableItem) {
    // Build prefix
    let mut line = String::new();

    for &is_last in &item.ancestors_are_last {
        if is_last {
            line.push_str("    ");
        } else {
            line.push_str("│   ");
        }
    }

    // Branch character
    if item.is_last_sibling {
        line.push_str("└── ");
    } else {
        line.push_str("├── ");
    }

    // Name with color codes
    if item.status.is_gitignored {
        line.push_str("\x1b[90m");
    }
    line.push_str(&item.name);
    if item.status.is_gitignored {
        line.push_str("\x1b[0m");
    }

    // Status indicators
    if item.status.is_staged {
        line.push_str("\x1b[32m ↑\x1b[0m");
    }
    if item.status.is_unstaged {
        line.push_str("\x1b[31m ✗\x1b[0m");
    }
    if item.status.is_untracked {
        line.push_str("\x1b[90m ✗\x1b[0m");
    }
    if item.status.has_incoming {
        line.push_str("\x1b[34m ↓\x1b[0m");
    }

    println!("{}", line);
}
