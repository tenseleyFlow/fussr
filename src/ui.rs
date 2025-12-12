use crate::app::App;
use crate::types::{AppMode, InputMode, SelectableItem};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
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
    let help_text = match &app.input_mode {
        InputMode::Rename { .. } => {
            Line::from(vec![
                Span::styled(
                    "RENAME: ",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw("Type new name | "),
                Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": save | "),
                Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": cancel"),
            ])
        }
        InputMode::Search { buffer } => {
            Line::from(vec![
                Span::styled(
                    "SEARCH: ",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw(buffer.clone()),
            ])
        }
        _ => {
            if app.mode == AppMode::Git {
                Line::from(vec![
                    Span::styled("GIT MODE: ", Style::default().fg(Color::Yellow)),
                    Span::raw("a:stage u:unstage S:stage-all U:unstage-all x:discard m:commit "),
                    Span::raw("f:fetch l:pull p:push d:diff q:exit-mode ESC:exit ctrl-c:quit"),
                ])
            } else {
                Line::from(vec![
                    Span::styled("↑", Style::default().fg(Color::Green)),
                    Span::raw("=staged "),
                    Span::styled("✗", Style::default().fg(Color::Red)),
                    Span::raw("=modified "),
                    Span::styled("✗", Style::default().fg(Color::DarkGray)),
                    Span::raw("=untracked "),
                    Span::styled("↓", Style::default().fg(Color::Blue)),
                    Span::raw("=incoming | j/k:nav ←/→:tree space:toggle .:dotfiles alt-g:git ctrl-c:quit"),
                ])
            }
        }
    };

    let legend = Line::from(vec![
        Span::styled("↑", Style::default().fg(Color::Green)),
        Span::raw("=staged "),
        Span::styled("✗", Style::default().fg(Color::Red)),
        Span::raw("=modified "),
        Span::styled("✗", Style::default().fg(Color::DarkGray)),
        Span::raw("=untracked "),
        Span::styled("↓", Style::default().fg(Color::Blue)),
        Span::raw("=incoming"),
    ]);

    let help = Paragraph::new(vec![legend, help_text]);
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
