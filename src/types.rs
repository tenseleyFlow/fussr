use std::path::PathBuf;

/// Status indicators for a file in git
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileStatus {
    pub is_staged: bool,
    pub is_unstaged: bool,
    pub is_untracked: bool,
    pub has_incoming: bool,
    pub is_gitignored: bool,
}

impl FileStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn staged() -> Self {
        Self {
            is_staged: true,
            ..Default::default()
        }
    }

    pub fn unstaged() -> Self {
        Self {
            is_unstaged: true,
            ..Default::default()
        }
    }

    pub fn untracked() -> Self {
        Self {
            is_untracked: true,
            ..Default::default()
        }
    }

    /// Merge status flags from another FileStatus
    pub fn merge(&mut self, other: &FileStatus) {
        self.is_staged |= other.is_staged;
        self.is_unstaged |= other.is_unstaged;
        self.is_untracked |= other.is_untracked;
        self.has_incoming |= other.has_incoming;
        self.is_gitignored |= other.is_gitignored;
    }

    /// Check if file has any modifications
    pub fn is_dirty(&self) -> bool {
        self.is_staged || self.is_unstaged || self.is_untracked
    }
}

/// A file entry from git status
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub status: FileStatus,
}

impl FileEntry {
    pub fn new(path: PathBuf, status: FileStatus) -> Self {
        Self { path, status }
    }
}

/// Node in the file tree (first-child, next-sibling representation)
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub full_path: PathBuf,
    pub is_file: bool,
    pub status: FileStatus,
    pub is_expanded: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new_directory(name: String, full_path: PathBuf) -> Self {
        Self {
            name,
            full_path,
            is_file: false,
            status: FileStatus::new(),
            is_expanded: true,
            children: Vec::new(),
        }
    }

    pub fn new_file(name: String, full_path: PathBuf, status: FileStatus) -> Self {
        Self {
            name,
            full_path,
            is_file: true,
            status,
            is_expanded: true, // Files are always "expanded"
            children: Vec::new(),
        }
    }

    pub fn root() -> Self {
        Self {
            name: ".".to_string(),
            full_path: PathBuf::from("."),
            is_file: false,
            status: FileStatus::new(),
            is_expanded: true,
            children: Vec::new(),
        }
    }

    /// Toggle expanded state (only meaningful for directories)
    pub fn toggle_expanded(&mut self) {
        if !self.is_file {
            self.is_expanded = !self.is_expanded;
        }
    }

    /// Sort children alphabetically
    pub fn sort_children(&mut self) {
        self.children
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        for child in &mut self.children {
            child.sort_children();
        }
    }
}

/// A flattened item for display and navigation
#[derive(Debug, Clone)]
pub struct SelectableItem {
    pub path: PathBuf,
    pub name: String,
    pub is_file: bool,
    pub status: FileStatus,
    pub depth: usize,
    pub is_expanded: bool,
    pub is_last_sibling: bool,
    /// Track ancestor "is_last" states for drawing tree lines
    pub ancestors_are_last: Vec<bool>,
}

impl SelectableItem {
    pub fn from_node(node: &TreeNode, depth: usize, is_last: bool, ancestors: Vec<bool>) -> Self {
        Self {
            path: node.full_path.clone(),
            name: node.name.clone(),
            is_file: node.is_file,
            status: node.status.clone(),
            depth,
            is_expanded: node.is_expanded,
            is_last_sibling: is_last,
            ancestors_are_last: ancestors,
        }
    }
}

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Git,
}

impl AppMode {
    pub fn toggle(&mut self) {
        *self = match self {
            AppMode::Normal => AppMode::Git,
            AppMode::Git => AppMode::Normal,
        };
    }
}

/// Status of commit operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitStatus {
    Editing,
    Committing,
    Success,
    Failed,
}

/// Status of push operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushStatus {
    SelectRemote,
    Pushing,
    Success,
    Failed(String),
}

/// Status of pull operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullStatus {
    SelectRemote,
    Pulling,
    Success,
    Failed(String),
}

/// Status of fetch operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    SelectRemote,
    Fetching,
    Success,
    Failed(String),
}

/// Status/step of tag operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagStep {
    EnterName,
    EnterMessage,
    Creating,
    AskPush,
    Pushing,
    Success,
    Failed(String),
}

/// Input mode for special states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Navigation,
    Rename { buffer: String, cursor: usize },
    Search { buffer: String },
    Commit { buffer: String, cursor: usize, amend: bool, status: CommitStatus },
    Push { remotes: Vec<String>, selected: usize, status: PushStatus },
    Pull { remotes: Vec<String>, selected: usize, status: PullStatus },
    Fetch { remotes: Vec<String>, selected: usize, status: FetchStatus },
    Tag { name: String, message: String, cursor: usize, existing_tags: Vec<String>, step: TagStep },
    Confirm { message: String, action: ConfirmAction },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Delete(PathBuf),
    Discard(PathBuf),
    StageAll,
    UnstageAll,
}
