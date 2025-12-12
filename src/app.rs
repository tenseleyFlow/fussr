use crate::error::Result;
use crate::git::GitRepo;
use crate::tree::{build_tree, flatten_tree, toggle_expanded};
use crate::types::{AppMode, FileEntry, InputMode, SelectableItem, TreeNode};
use std::path::PathBuf;

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
        };
    }

    /// Apply commit
    pub fn apply_commit(&mut self) -> Result<()> {
        if let InputMode::Commit { buffer, amend, .. } = &self.input_mode {
            let message = buffer.trim();
            if !message.is_empty() {
                if *amend {
                    self.repo.commit_amend(message)?;
                    self.set_status("Commit amended".to_string());
                } else {
                    self.repo.commit(message)?;
                    self.set_status("Committed".to_string());
                }
                self.refresh_files()?;
            }
        }
        self.input_mode = InputMode::Navigation;
        Ok(())
    }

    /// Cancel commit mode
    pub fn cancel_commit(&mut self) {
        self.input_mode = InputMode::Navigation;
    }

    /// Fuzzy search and jump to match
    pub fn fuzzy_jump(&mut self, pattern: &str) {
        if pattern.is_empty() {
            return;
        }

        let pattern_lower = pattern.to_lowercase();
        let mut best_idx = self.selected;
        let mut best_score = 0;

        for (i, item) in self.items.iter().enumerate() {
            let score = fuzzy_score(&pattern_lower, &item.name.to_lowercase());
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }

        if best_score > 0 {
            self.selected = best_idx;
        }
    }
}

/// Simple fuzzy matching score
fn fuzzy_score(pattern: &str, text: &str) -> usize {
    if text.starts_with(pattern) {
        return 1000 + (100 - text.len()); // Prefix match bonus
    }

    let mut score = 0;
    let mut pattern_idx = 0;
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let mut consecutive = 0;

    for (i, c) in text.chars().enumerate() {
        if pattern_idx < pattern_chars.len() && c == pattern_chars[pattern_idx] {
            score += 10 + consecutive * 5;
            if i == 0 {
                score += 20; // Start match bonus
            }
            consecutive += 1;
            pattern_idx += 1;
        } else {
            consecutive = 0;
        }
    }

    if pattern_idx == pattern_chars.len() {
        score
    } else {
        0 // Not all chars matched
    }
}
