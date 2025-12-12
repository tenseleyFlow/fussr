use crate::error::Result;
use crate::git::GitRepo;
use crate::tree::{build_tree, flatten_tree, toggle_expanded};
use crate::types::{AppMode, FileEntry, InputMode, SelectableItem, TreeNode};
use std::path::PathBuf;
use std::time::Instant;

/// Main application state
pub struct App {
    /// Git repository
    pub repo: GitRepo,
    /// Repository name
    pub repo_name: String,
    /// Current branch name
    pub branch_name: String,
    /// Whether to show all files or just dirty ones
    pub show_all: bool,
    /// Whether to hide dotfiles
    pub hide_dotfiles: bool,
    /// Tree root
    pub tree: TreeNode,
    /// Flattened items for display
    pub items: Vec<SelectableItem>,
    /// Currently selected item index
    pub selected: usize,
    /// Viewport offset for scrolling
    pub viewport_offset: usize,
    /// Application mode (Normal or Git)
    pub mode: AppMode,
    /// Input mode (Navigation, Rename, Search, etc.)
    pub input_mode: InputMode,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Status message to display
    pub status_message: Option<String>,
    /// Fuzzy search buffer
    pub search_buffer: String,
    /// Last search keystroke time (for timeout)
    pub last_search_time: Option<Instant>,
}


impl App {
    /// Create a new application instance
    pub fn new(show_all: bool) -> Result<Self> {
        let repo = GitRepo::open()?;
        let repo_name = repo.repo_name();
        let branch_name = repo.branch_name();

        let mut app = Self {
            repo,
            repo_name,
            branch_name,
            show_all,
            hide_dotfiles: false,
            tree: TreeNode::root(),
            items: Vec::new(),
            selected: 0,
            viewport_offset: 0,
            mode: AppMode::Normal,
            input_mode: InputMode::Navigation,
            should_quit: false,
            status_message: None,
            search_buffer: String::new(),
            last_search_time: None,
        };

        app.refresh_files()?;
        Ok(app)
    }

    /// Refresh file list and rebuild tree
    pub fn refresh_files(&mut self) -> Result<()> {
        let mut files = if self.show_all {
            self.repo.get_all_files()?
        } else {
            self.repo.get_dirty_files()?
        };

        // Mark incoming changes
        let _ = self.repo.mark_incoming_changes(&mut files);

        self.rebuild_tree(&files);
        self.update_branch_info();
        Ok(())
    }

    /// Rebuild tree from file entries, preserving expanded state
    fn rebuild_tree(&mut self, files: &[FileEntry]) {
        // Save collapsed paths
        let collapsed_paths = self.get_collapsed_paths();

        // Build new tree
        self.tree = build_tree(files, self.hide_dotfiles);

        // Restore collapsed state
        self.restore_collapsed_paths(&collapsed_paths);

        // Flatten for display
        self.items = flatten_tree(&self.tree, self.hide_dotfiles);

        // Adjust selection if needed
        if self.selected >= self.items.len() && !self.items.is_empty() {
            self.selected = self.items.len() - 1;
        }
    }

    /// Get paths of all collapsed directories
    fn get_collapsed_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        Self::collect_collapsed(&self.tree, &mut paths);
        paths
    }

    fn collect_collapsed(node: &TreeNode, paths: &mut Vec<PathBuf>) {
        if !node.is_file && !node.is_expanded && node.name != "." {
            paths.push(node.full_path.clone());
        }
        for child in &node.children {
            Self::collect_collapsed(child, paths);
        }
    }

    /// Restore collapsed state from saved paths
    fn restore_collapsed_paths(&mut self, paths: &[PathBuf]) {
        for path in paths {
            Self::set_collapsed(&mut self.tree, path);
        }
    }

    fn set_collapsed(node: &mut TreeNode, path: &PathBuf) {
        if &node.full_path == path {
            node.is_expanded = false;
            return;
        }
        for child in &mut node.children {
            Self::set_collapsed(child, path);
        }
    }

    /// Update branch info
    pub fn update_branch_info(&mut self) {
        self.repo_name = self.repo.repo_name();
        self.branch_name = self.repo.branch_name();
    }

    /// Get currently selected item
    pub fn selected_item(&self) -> Option<&SelectableItem> {
        self.items.get(self.selected)
    }

    /// Navigate down in the list
    pub fn navigate_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    /// Navigate up in the list
    pub fn navigate_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Navigate left (collapse current directory or go to parent)
    pub fn navigate_left(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let item = &self.items[self.selected];

        // If it's an expanded directory, collapse it
        if !item.is_file && item.is_expanded {
            self.toggle_selected();
            return;
        }

        // Otherwise, go to parent directory
        let current_depth = item.depth;
        if current_depth == 0 {
            return; // Already at root level
        }

        // Find parent (previous item at depth - 1)
        for i in (0..self.selected).rev() {
            if self.items[i].depth == current_depth - 1 {
                self.selected = i;
                return;
            }
        }
    }

    /// Navigate right (expand directory or go into it)
    pub fn navigate_right(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let item = &self.items[self.selected];
        if item.is_file {
            return; // Can't enter a file
        }

        // If collapsed, expand
        if !item.is_expanded {
            self.toggle_selected();
            return;
        }

        // If expanded, move to first child
        let target_depth = item.depth + 1;
        for i in (self.selected + 1)..self.items.len() {
            if self.items[i].depth == target_depth {
                self.selected = i;
                return;
            }
        }
    }

    /// Toggle expanded state of selected directory
    pub fn toggle_selected(&mut self) {
        if let Some(item) = self.items.get(self.selected) {
            if !item.is_file {
                let path = item.path.clone();
                if toggle_expanded(&mut self.tree, &path) {
                    self.items = flatten_tree(&self.tree, self.hide_dotfiles);
                    // Keep selection valid
                    if self.selected >= self.items.len() && !self.items.is_empty() {
                        self.selected = self.items.len() - 1;
                    }
                }
            }
        }
    }

    /// Toggle dotfile visibility
    pub fn toggle_dotfiles(&mut self) {
        self.hide_dotfiles = !self.hide_dotfiles;
        self.items = flatten_tree(&self.tree, self.hide_dotfiles);
        if self.selected >= self.items.len() && !self.items.is_empty() {
            self.selected = self.items.len() - 1;
        }
    }

    /// Toggle app mode (Normal <-> Git)
    pub fn toggle_mode(&mut self) {
        self.mode.toggle();
    }

    /// Stage selected file or directory
    pub fn stage_selected(&mut self) -> Result<()> {
        if let Some(item) = self.selected_item().cloned() {
            if item.is_file {
                if item.status.is_unstaged || item.status.is_untracked {
                    self.repo.stage_file(&item.path)?;
                    self.set_status(format!("Staged: {}", item.path.display()));
                }
            } else {
                self.repo.stage_directory(&item.path)?;
                self.set_status(format!("Staged directory: {}", item.path.display()));
            }
            self.refresh_files()?;
        }
        Ok(())
    }

    /// Unstage selected file
    pub fn unstage_selected(&mut self) -> Result<()> {
        if let Some(item) = self.selected_item().cloned() {
            if item.is_file && item.status.is_staged {
                self.repo.unstage_file(&item.path)?;
                self.set_status(format!("Unstaged: {}", item.path.display()));
                self.refresh_files()?;
            }
        }
        Ok(())
    }

    /// Stage all files
    pub fn stage_all(&mut self) -> Result<()> {
        self.repo.stage_all()?;
        self.set_status("Staged all changes".to_string());
        self.refresh_files()
    }

    /// Unstage all files
    pub fn unstage_all(&mut self) -> Result<()> {
        self.repo.unstage_all()?;
        self.set_status("Unstaged all files".to_string());
        self.refresh_files()
    }

    /// Discard changes to selected file
    pub fn discard_selected(&mut self) -> Result<()> {
        if let Some(item) = self.selected_item().cloned() {
            if item.is_file && item.status.is_dirty() {
                self.repo.discard_changes(&item.path, item.status.is_untracked)?;
                self.set_status(format!("Discarded changes: {}", item.path.display()));
                self.refresh_files()?;
            }
        }
        Ok(())
    }

    /// Delete selected file
    pub fn delete_selected(&mut self) -> Result<()> {
        if let Some(item) = self.selected_item().cloned() {
            if item.is_file {
                self.repo.delete_file(&item.path, item.status.is_untracked)?;
                self.set_status(format!("Deleted: {}", item.path.display()));
                self.refresh_files()?;
            }
        }
        Ok(())
    }

    /// Fetch from remote
    pub fn fetch(&mut self) -> Result<()> {
        self.set_status("Fetching...".to_string());
        self.repo.fetch()?;
        self.set_status("Fetch complete".to_string());
        self.refresh_files()
    }

    /// Pull from remote
    pub fn pull(&mut self) -> Result<()> {
        self.set_status("Pulling...".to_string());
        self.repo.pull()?;
        self.set_status("Pull complete".to_string());
        self.refresh_files()
    }

    /// Push to remote
    pub fn push(&mut self) -> Result<()> {
        self.set_status("Pushing...".to_string());
        self.repo.push()?;
        self.set_status("Push complete".to_string());
        Ok(())
    }

    /// Create a commit
    pub fn commit(&mut self, message: &str) -> Result<()> {
        self.repo.commit(message)?;
        self.set_status("Committed successfully".to_string());
        self.refresh_files()
    }

    /// Amend the last commit
    pub fn commit_amend(&mut self, message: &str) -> Result<()> {
        self.repo.commit_amend(message)?;
        self.set_status("Commit amended".to_string());
        self.refresh_files()
    }

    /// Get last commit message
    pub fn last_commit_message(&self) -> Option<String> {
        self.repo.last_commit_message()
    }

    /// Set status message
    pub fn set_status(&mut self, message: String) {
        self.status_message = Some(message);
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Enter rename mode for selected item
    pub fn enter_rename_mode(&mut self) {
        if let Some(item) = self.selected_item() {
            self.input_mode = InputMode::Rename {
                buffer: item.name.clone(),
                cursor: item.name.len(),
            };
        }
    }

    /// Exit rename mode and apply rename
    pub fn apply_rename(&mut self) -> Result<()> {
        if let InputMode::Rename { buffer, .. } = &self.input_mode {
            if let Some(item) = self.selected_item().cloned() {
                let new_name = buffer.trim();
                if !new_name.is_empty() && new_name != item.name {
                    let new_path = item.path.with_file_name(new_name);
                    self.repo.rename_file(&item.path, &new_path)?;
                    self.set_status(format!("Renamed to: {}", new_name));
                    self.refresh_files()?;
                }
            }
        }
        self.input_mode = InputMode::Navigation;
        Ok(())
    }

    /// Cancel rename mode
    pub fn cancel_rename(&mut self) {
        self.input_mode = InputMode::Navigation;
    }

    /// Enter commit mode
    pub fn enter_commit_mode(&mut self, amend: bool) {
        let initial_buffer = if amend {
            self.repo.last_commit_message().unwrap_or_default()
        } else {
            String::new()
        };
        let cursor = initial_buffer.len();
        self.input_mode = InputMode::Commit {
            buffer: initial_buffer,
            cursor,
            amend,
            status: crate::types::CommitStatus::Editing,
        };
    }

    /// Start the commit process (show "Committing..." state)
    pub fn start_commit(&mut self) {
        if let InputMode::Commit { status, .. } = &mut self.input_mode {
            *status = crate::types::CommitStatus::Committing;
        }
    }

    /// Apply commit and update status
    pub fn apply_commit(&mut self) -> Result<()> {
        // Extract values we need before modifying
        let (message, amend) = if let InputMode::Commit { buffer, amend, .. } = &self.input_mode {
            (buffer.trim().to_string(), *amend)
        } else {
            return Ok(());
        };

        if message.is_empty() {
            self.input_mode = InputMode::Navigation;
            return Ok(());
        }

        // Perform the commit
        let result = if amend {
            self.repo.commit_amend(&message)
        } else {
            self.repo.commit(&message)
        };

        // Update status based on result
        match result {
            Ok(()) => {
                if let InputMode::Commit { status, .. } = &mut self.input_mode {
                    *status = crate::types::CommitStatus::Success;
                }
                self.set_status("Committed".to_string());
                self.refresh_files()?;
            }
            Err(_) => {
                if let InputMode::Commit { status, .. } = &mut self.input_mode {
                    *status = crate::types::CommitStatus::Failed;
                }
            }
        }

        Ok(())
    }

    /// Close commit modal and return to navigation
    pub fn close_commit(&mut self) {
        self.input_mode = InputMode::Navigation;
    }

    /// Cancel commit mode
    pub fn cancel_commit(&mut self) {
        self.input_mode = InputMode::Navigation;
    }

    /// Check if search timeout has elapsed (0.5 seconds) and reset if needed
    pub fn check_search_timeout(&mut self) {
        if let Some(last_time) = self.last_search_time {
            if last_time.elapsed().as_millis() > 500 {
                self.search_buffer.clear();
                self.last_search_time = None;
            }
        }
    }

    /// Add a character to the search buffer and jump to match
    pub fn search_add_char(&mut self, c: char) {
        // Check timeout first - if elapsed, start fresh
        self.check_search_timeout();

        // Add character to buffer
        if self.search_buffer.len() < 32 {
            self.search_buffer.push(c);
            self.last_search_time = Some(Instant::now());

            // Jump to best match
            self.fuzzy_jump_to_match();
        }
    }

    /// Remove last character from search buffer
    pub fn search_backspace(&mut self) {
        if !self.search_buffer.is_empty() {
            self.search_buffer.pop();
            self.last_search_time = Some(Instant::now());

            if !self.search_buffer.is_empty() {
                self.fuzzy_jump_to_match();
            }
        }
    }

    /// Clear the search buffer
    pub fn clear_search(&mut self) {
        self.search_buffer.clear();
        self.last_search_time = None;
    }

    /// Jump to the best matching item using fzf-style scoring
    fn fuzzy_jump_to_match(&mut self) {
        if self.search_buffer.is_empty() || self.items.is_empty() {
            return;
        }

        let pattern = self.search_buffer.to_lowercase();
        let mut best_idx = self.selected;
        let mut best_score: i32 = 0;

        // Check current item first - if exact match, stay on it
        if let Some(item) = self.items.get(self.selected) {
            let current_score = fuzzy_match_score(&pattern, &item.name.to_lowercase());
            if current_score >= 10000 {
                return; // Exact match - stay here
            }
            best_score = current_score;
        }

        // PASS 1: Search for basename matches (file/folder names)
        for (i, item) in self.items.iter().enumerate() {
            if i == self.selected {
                continue;
            }
            let score = fuzzy_match_score(&pattern, &item.name.to_lowercase());
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }

        // If we found a good basename match (prefix or exact), use it
        if best_score >= 5000 {
            self.selected = best_idx;
            return;
        }

        // PASS 2: Search full paths if no good basename match
        for (i, item) in self.items.iter().enumerate() {
            if i == self.selected {
                continue;
            }
            let path_str = item.path.to_string_lossy().to_lowercase();
            let score = fuzzy_match_score(&pattern, &path_str);
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }

        // Jump to best match if any was found
        if best_score > 0 {
            self.selected = best_idx;
        }
    }
}

/// Fuzzy matching with fzf-style scoring
/// Returns a score (higher is better), 0 means no match
fn fuzzy_match_score(pattern: &str, text: &str) -> i32 {
    if pattern.is_empty() {
        return 1;
    }

    // Exact match (highest score)
    if pattern == text {
        return 10000;
    }

    // Prefix match (very high score)
    if text.starts_with(pattern) {
        return 5000;
    }

    // Fuzzy match with scoring
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    let mut score: i32 = 0;
    let mut pattern_idx = 0;
    let mut consecutive_bonus: i32 = 0;
    let mut is_consecutive = false;
    let mut match_started = false;

    for (text_idx, &c) in text_chars.iter().enumerate() {
        if pattern_idx >= pattern_chars.len() {
            break;
        }

        if c == pattern_chars[pattern_idx] {
            match_started = true;

            // Base score for each matched character
            score += 100;

            // Bonus for consecutive characters
            if is_consecutive {
                consecutive_bonus += 1;
                score += consecutive_bonus * 50;
            } else {
                consecutive_bonus = 1;
                is_consecutive = true;
            }

            // Bonus for matching at start of text
            if text_idx == 0 {
                score += 200;
            }

            // Bonus for matching after separator (word boundary)
            if text_idx > 0 {
                let prev = text_chars[text_idx - 1];
                if prev == '/' || prev == '_' || prev == '-' || prev == '.' {
                    score += 150;
                }
            }

            pattern_idx += 1;
        } else {
            // Reset consecutive bonus
            is_consecutive = false;
            consecutive_bonus = 0;
            // Small penalty for gaps
            if match_started {
                score -= 1;
            }
        }
    }

    // No match if we didn't find all pattern characters
    if pattern_idx < pattern_chars.len() {
        return 0;
    }

    // Penalty for longer strings (prefer concise matches)
    score -= text.len() as i32;

    score.max(1) // Ensure positive score if we matched
}
