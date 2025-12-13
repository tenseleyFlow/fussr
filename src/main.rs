mod app;
mod cli;
mod error;
mod git;
mod tree;
mod types;
mod ui;

use crate::app::App;
use crate::cli::Args;
use crate::error::Result;
use crate::types::{AppMode, InputMode};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

fn main() -> Result<()> {
    let args = Args::parse_args();

    // Create app
    let mut app = App::new(args.show_all_files())?;

    if app.items.is_empty() {
        println!("No files to display");
        return Ok(());
    }

    if args.is_interactive() {
        run_interactive(&mut app)?;
    } else {
        ui::print_tree(&app);
    }

    Ok(())
}

/// Run the interactive TUI
fn run_interactive(app: &mut App) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run event loop
    let result = run_event_loop(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Print final tree state
    println!(".");
    ui::print_tree(app);

    result
}

/// Main event loop
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle Ctrl+Q to quit from any mode
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q')
                {
                    app.should_quit = true;
                }

                // Handle key based on current input mode
                match &app.input_mode {
                    InputMode::Navigation => handle_navigation_key(app, key.code, key.modifiers)?,
                    InputMode::Rename { .. } => handle_rename_key(app, key.code)?,
                    InputMode::Commit { .. } => handle_commit_key(app, key.code)?,
                    InputMode::Search { .. } => handle_search_key(app, key.code)?,
                    InputMode::Push { .. } => handle_push_key(app, key.code)?,
                    InputMode::Pull { .. } => handle_pull_key(app, key.code)?,
                    InputMode::Fetch { .. } => handle_fetch_key(app, key.code)?,
                    InputMode::Tag { .. } => handle_tag_key(app, key.code)?,
                    InputMode::Confirm { .. } => handle_confirm_key(app, key.code)?,
                }

                if app.should_quit {
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Handle keys in navigation mode
fn handle_navigation_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
    // Check search timeout on every key
    app.check_search_timeout();

    // Check for Alt+key combinations
    if modifiers.contains(KeyModifiers::ALT) {
        match code {
            KeyCode::Char('g') => app.toggle_mode(),
            KeyCode::Char('n') => app.enter_rename_mode(),
            _ => {}
        }
        return Ok(());
    }

    // In normal mode, handle fuzzy search for printable characters
    if app.mode == AppMode::Normal {
        match code {
            // Fuzzy search - letters, numbers, and common filename chars
            KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {
                app.search_add_char(c);
                return Ok(());
            }
            // Backspace removes from search buffer
            KeyCode::Backspace => {
                if !app.search_buffer.is_empty() {
                    app.search_backspace();
                    return Ok(());
                }
            }
            // ESC clears search buffer in normal mode
            KeyCode::Esc => {
                if !app.search_buffer.is_empty() {
                    app.clear_search();
                    return Ok(());
                }
            }
            _ => {}
        }
    }

    match code {
        // Navigation (clears search buffer)
        KeyCode::Char('j') | KeyCode::Down => {
            app.clear_search();
            app.navigate_down();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.clear_search();
            app.navigate_up();
        }
        KeyCode::Left => {
            app.clear_search();
            app.navigate_left();
        }
        KeyCode::Right => {
            app.clear_search();
            app.navigate_right();
        }
        KeyCode::Char(' ') => {
            app.clear_search();
            app.toggle_selected();
        }
        KeyCode::Char('.') => app.toggle_dotfiles(),

        // Mode switching
        KeyCode::Char('q') | KeyCode::Char('Q') if app.mode == AppMode::Git => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Esc if app.mode == AppMode::Git => {
            app.mode = AppMode::Normal;
        }

        // Git mode commands
        KeyCode::Char('a') if app.mode == AppMode::Git => {
            app.stage_selected()?;
        }
        KeyCode::Char('u') if app.mode == AppMode::Git => {
            app.unstage_selected()?;
        }
        KeyCode::Char('S') if app.mode == AppMode::Git => {
            app.stage_all()?;
        }
        KeyCode::Char('U') if app.mode == AppMode::Git => {
            app.unstage_all()?;
        }
        KeyCode::Char('x') | KeyCode::Char('X') if app.mode == AppMode::Git => {
            app.confirm_discard();
        }
        KeyCode::Char('r') if app.mode == AppMode::Git => {
            app.delete_selected()?;
        }
        KeyCode::Char('f') if app.mode == AppMode::Git => {
            match app.fetch() {
                Ok(()) => {}
                Err(e) => app.set_status(format!("Fetch failed: {}", e)),
            }
        }
        KeyCode::Char('F') if app.mode == AppMode::Git => {
            // Debug: Force show fetch modal regardless of remote count
            app.show_fetch_modal();
        }
        KeyCode::Char('l') if app.mode == AppMode::Git => {
            match app.pull() {
                Ok(()) => {}
                Err(e) => app.set_status(format!("Pull failed: {}", e)),
            }
        }
        KeyCode::Char('L') if app.mode == AppMode::Git => {
            // Debug: Force show pull modal regardless of upstream status
            app.show_pull_modal();
        }
        KeyCode::Char('p') if app.mode == AppMode::Git => {
            match app.push() {
                Ok(()) => {}
                Err(e) => app.set_status(format!("Push failed: {}", e)),
            }
        }
        KeyCode::Char('P') if app.mode == AppMode::Git => {
            // Debug: Force show push modal regardless of upstream status
            app.show_push_modal();
        }
        KeyCode::Char('m') if app.mode == AppMode::Git => {
            app.enter_commit_mode(false);
        }
        KeyCode::Char('M') if app.mode == AppMode::Git => {
            app.enter_commit_mode(true); // amend
        }
        KeyCode::Char('t') if app.mode == AppMode::Git => {
            app.enter_tag_mode();
        }

        _ => {}
    }

    Ok(())
}

/// Handle keys in rename mode
fn handle_rename_key(app: &mut App, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Esc => app.cancel_rename(),
        KeyCode::Tab | KeyCode::Enter => app.apply_rename()?,
        KeyCode::Backspace => {
            if let InputMode::Rename { buffer, cursor } = &mut app.input_mode {
                if *cursor > 0 {
                    buffer.remove(*cursor - 1);
                    *cursor -= 1;
                }
            }
        }
        KeyCode::Left => {
            if let InputMode::Rename { cursor, .. } = &mut app.input_mode {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
        }
        KeyCode::Right => {
            if let InputMode::Rename { buffer, cursor } = &mut app.input_mode {
                if *cursor < buffer.len() {
                    *cursor += 1;
                }
            }
        }
        KeyCode::Char(c) => {
            if let InputMode::Rename { buffer, cursor } = &mut app.input_mode {
                buffer.insert(*cursor, c);
                *cursor += 1;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle keys in commit mode
fn handle_commit_key(app: &mut App, code: KeyCode) -> Result<()> {
    use crate::types::CommitStatus;

    // Get current status
    let status = if let InputMode::Commit { status, .. } = &app.input_mode {
        status.clone()
    } else {
        return Ok(());
    };

    match status {
        CommitStatus::Editing => {
            // Normal editing mode
            match code {
                KeyCode::Esc => app.cancel_commit(),
                KeyCode::Enter => app.apply_commit()?,
                KeyCode::Backspace => {
                    if let InputMode::Commit { buffer, cursor, .. } = &mut app.input_mode {
                        if *cursor > 0 {
                            buffer.remove(*cursor - 1);
                            *cursor -= 1;
                        }
                    }
                }
                KeyCode::Left => {
                    if let InputMode::Commit { cursor, .. } = &mut app.input_mode {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                }
                KeyCode::Right => {
                    if let InputMode::Commit { buffer, cursor, .. } = &mut app.input_mode {
                        if *cursor < buffer.len() {
                            *cursor += 1;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let InputMode::Commit { buffer, cursor, .. } = &mut app.input_mode {
                        buffer.insert(*cursor, c);
                        *cursor += 1;
                    }
                }
                _ => {}
            }
        }
        CommitStatus::Committing => {
            // Don't respond to keys while committing
        }
        CommitStatus::Success | CommitStatus::Failed => {
            // Any key closes the modal
            app.close_commit();
        }
    }
    Ok(())
}

/// Handle keys in push mode
fn handle_push_key(app: &mut App, code: KeyCode) -> Result<()> {
    use crate::types::PushStatus;

    // Get current status
    let status = if let InputMode::Push { status, .. } = &app.input_mode {
        status.clone()
    } else {
        return Ok(());
    };

    match status {
        PushStatus::SelectRemote => {
            match code {
                KeyCode::Esc => app.close_push(),
                KeyCode::Char('j') | KeyCode::Down => {
                    if let InputMode::Push { remotes, selected, .. } = &mut app.input_mode {
                        if *selected + 1 < remotes.len() {
                            *selected += 1;
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if let InputMode::Push { selected, .. } = &mut app.input_mode {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    // Get the selected remote and push
                    let remote = if let InputMode::Push { remotes, selected, .. } = &app.input_mode {
                        remotes.get(*selected).cloned()
                    } else {
                        None
                    };

                    if let Some(remote) = remote {
                        app.push_to_remote(&remote)?;
                    }
                }
                _ => {}
            }
        }
        PushStatus::Pushing => {
            // Don't respond to keys while pushing
        }
        PushStatus::Success | PushStatus::Failed(_) => {
            // Any key closes the modal
            app.close_push();
        }
    }

    Ok(())
}

/// Handle keys in pull mode
fn handle_pull_key(app: &mut App, code: KeyCode) -> Result<()> {
    use crate::types::PullStatus;

    let status = if let InputMode::Pull { status, .. } = &app.input_mode {
        status.clone()
    } else {
        return Ok(());
    };

    match status {
        PullStatus::SelectRemote => {
            match code {
                KeyCode::Esc => app.close_pull(),
                KeyCode::Char('j') | KeyCode::Down => {
                    if let InputMode::Pull { remotes, selected, .. } = &mut app.input_mode {
                        if *selected + 1 < remotes.len() {
                            *selected += 1;
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if let InputMode::Pull { selected, .. } = &mut app.input_mode {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    let remote = if let InputMode::Pull { remotes, selected, .. } = &app.input_mode {
                        remotes.get(*selected).cloned()
                    } else {
                        None
                    };

                    if let Some(remote) = remote {
                        app.pull_from_remote(&remote)?;
                    }
                }
                _ => {}
            }
        }
        PullStatus::Pulling => {
            // Don't respond to keys while pulling
        }
        PullStatus::Success | PullStatus::Failed(_) => {
            // Any key closes the modal
            app.close_pull();
        }
    }

    Ok(())
}

/// Handle keys in fetch mode
fn handle_fetch_key(app: &mut App, code: KeyCode) -> Result<()> {
    use crate::types::FetchStatus;

    let status = if let InputMode::Fetch { status, .. } = &app.input_mode {
        status.clone()
    } else {
        return Ok(());
    };

    match status {
        FetchStatus::SelectRemote => {
            match code {
                KeyCode::Esc => app.close_fetch(),
                KeyCode::Char('j') | KeyCode::Down => {
                    if let InputMode::Fetch { remotes, selected, .. } = &mut app.input_mode {
                        if *selected + 1 < remotes.len() {
                            *selected += 1;
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if let InputMode::Fetch { selected, .. } = &mut app.input_mode {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    let remote = if let InputMode::Fetch { remotes, selected, .. } = &app.input_mode {
                        remotes.get(*selected).cloned()
                    } else {
                        None
                    };

                    if let Some(remote) = remote {
                        app.fetch_from_remote(&remote)?;
                    }
                }
                _ => {}
            }
        }
        FetchStatus::Fetching => {
            // Don't respond to keys while fetching
        }
        FetchStatus::Success | FetchStatus::Failed(_) => {
            // Any key closes the modal
            app.close_fetch();
        }
    }

    Ok(())
}

/// Handle keys in tag mode
fn handle_tag_key(app: &mut App, code: KeyCode) -> Result<()> {
    use crate::types::TagStep;

    let step = if let InputMode::Tag { step, .. } = &app.input_mode {
        step.clone()
    } else {
        return Ok(());
    };

    match step {
        TagStep::EnterName => {
            match code {
                KeyCode::Esc => {
                    app.close_tag(false);
                }
                KeyCode::Enter => {
                    // Check if name is non-empty before proceeding
                    let has_name = if let InputMode::Tag { name, .. } = &app.input_mode {
                        !name.trim().is_empty()
                    } else {
                        false
                    };

                    if has_name {
                        // Move cursor to message field
                        if let InputMode::Tag { step, cursor, .. } = &mut app.input_mode {
                            *step = TagStep::EnterMessage;
                            *cursor = 0;
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let InputMode::Tag { name, cursor, .. } = &mut app.input_mode {
                        if *cursor > 0 {
                            name.remove(*cursor - 1);
                            *cursor -= 1;
                        }
                    }
                }
                KeyCode::Left => {
                    if let InputMode::Tag { cursor, .. } = &mut app.input_mode {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                }
                KeyCode::Right => {
                    if let InputMode::Tag { name, cursor, .. } = &mut app.input_mode {
                        if *cursor < name.len() {
                            *cursor += 1;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let InputMode::Tag { name, cursor, .. } = &mut app.input_mode {
                        name.insert(*cursor, c);
                        *cursor += 1;
                    }
                }
                _ => {}
            }
        }
        TagStep::EnterMessage => {
            match code {
                KeyCode::Esc => {
                    // Go back to name entry
                    if let InputMode::Tag { step, name, cursor, .. } = &mut app.input_mode {
                        *step = TagStep::EnterName;
                        *cursor = name.len();
                    }
                }
                KeyCode::Enter => {
                    // Create the tag
                    app.create_tag()?;
                }
                KeyCode::Backspace => {
                    if let InputMode::Tag { message, cursor, .. } = &mut app.input_mode {
                        if *cursor > 0 {
                            message.remove(*cursor - 1);
                            *cursor -= 1;
                        }
                    }
                }
                KeyCode::Left => {
                    if let InputMode::Tag { cursor, .. } = &mut app.input_mode {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                }
                KeyCode::Right => {
                    if let InputMode::Tag { message, cursor, .. } = &mut app.input_mode {
                        if *cursor < message.len() {
                            *cursor += 1;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let InputMode::Tag { message, cursor, .. } = &mut app.input_mode {
                        message.insert(*cursor, c);
                        *cursor += 1;
                    }
                }
                _ => {}
            }
        }
        TagStep::Creating | TagStep::Pushing => {
            // Don't respond to keys while creating/pushing
        }
        TagStep::AskPush => {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.push_tag()?;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.close_tag(true);
                }
                _ => {}
            }
        }
        TagStep::Success => {
            // Any key closes
            app.close_tag(true);
        }
        TagStep::Failed(_) => {
            // Any key closes
            app.close_tag(false);
        }
    }

    Ok(())
}

/// Handle keys in search mode (legacy - not currently used)
fn handle_search_key(app: &mut App, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Navigation;
        }
        KeyCode::Backspace => {
            if let InputMode::Search { buffer } = &mut app.input_mode {
                buffer.pop();
            }
        }
        KeyCode::Char(c) => {
            if let InputMode::Search { buffer } = &mut app.input_mode {
                buffer.push(c);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle keys in confirm mode
fn handle_confirm_key(app: &mut App, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            // Extract action and execute
            if let InputMode::Confirm { action, .. } = &app.input_mode {
                let action = action.clone();
                app.input_mode = InputMode::Navigation;
                app.execute_confirm_action(&action)?;
            }
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            app.input_mode = InputMode::Navigation;
        }
        _ => {}
    }
    Ok(())
}
