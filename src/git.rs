use crate::error::{FussrError, Result};
use crate::types::{FileEntry, FileStatus};
use git2::{Repository, Status, StatusOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git operations wrapper
pub struct GitRepo {
    repo: Repository,
}

impl GitRepo {
    /// Open the git repository in the current directory
    pub fn open() -> Result<Self> {
        let repo = Repository::discover(".")?;
        Ok(Self { repo })
    }

    /// Get repository name (basename of repo root)
    pub fn repo_name(&self) -> String {
        self.repo
            .workdir()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Get current branch name
    pub fn branch_name(&self) -> String {
        self.repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "HEAD".to_string())
    }

    /// Get dirty files (staged, unstaged, untracked)
    pub fn get_dirty_files(&self) -> Result<Vec<FileEntry>> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut files = Vec::new();

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let status = self.parse_status(entry.status());
                files.push(FileEntry::new(PathBuf::from(path), status));
            }
        }

        // Sort by path for consistent display
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Get all tracked files (optionally including dirty status)
    pub fn get_all_files(&self) -> Result<Vec<FileEntry>> {
        // First get dirty files for status lookup
        let dirty_files = self.get_dirty_files()?;
        let dirty_paths: std::collections::HashMap<_, _> = dirty_files
            .iter()
            .map(|f| (f.path.clone(), f.status.clone()))
            .collect();

        // Get all tracked files from index
        let index = self.repo.index()?;
        let mut files = Vec::new();
        let mut seen = HashSet::new();

        for entry in index.iter() {
            let path = PathBuf::from(
                std::str::from_utf8(&entry.path).unwrap_or_default()
            );

            if seen.insert(path.clone()) {
                let status = dirty_paths
                    .get(&path)
                    .cloned()
                    .unwrap_or_default();
                files.push(FileEntry::new(path, status));
            }
        }

        // Add untracked files from dirty list
        for file in dirty_files {
            if file.status.is_untracked && !seen.contains(&file.path) {
                files.push(file);
            }
        }

        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Parse git2 Status flags into our FileStatus
    fn parse_status(&self, status: Status) -> FileStatus {
        let mut fs = FileStatus::new();

        // Index (staged) changes
        if status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        ) {
            fs.is_staged = true;
        }

        // Worktree (unstaged) changes
        if status.intersects(
            Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE,
        ) {
            fs.is_unstaged = true;
        }

        // Untracked
        if status.contains(Status::WT_NEW) {
            fs.is_untracked = true;
        }

        // Ignored
        if status.contains(Status::IGNORED) {
            fs.is_gitignored = true;
        }

        fs
    }

    /// Mark files that have incoming changes from remote
    pub fn mark_incoming_changes(&self, files: &mut [FileEntry]) -> Result<()> {
        // Check if there's an upstream branch
        let upstream = match self.get_upstream_name() {
            Some(name) => name,
            None => return Ok(()), // No upstream, nothing to do
        };

        // Get files changed between HEAD and upstream using git diff
        let output = Command::new("git")
            .args(["diff", "--name-only", &format!("HEAD...{}", upstream)])
            .output()?;

        if !output.status.success() {
            return Ok(());
        }

        let changed: HashSet<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(PathBuf::from)
            .collect();

        for file in files {
            if changed.contains(&file.path) {
                file.status.has_incoming = true;
            }
        }

        Ok(())
    }

    /// Get upstream branch name if configured
    fn get_upstream_name(&self) -> Option<String> {
        let head = self.repo.head().ok()?;
        let branch_name = head.shorthand()?;
        let branch = self.repo.find_branch(branch_name, git2::BranchType::Local).ok()?;
        let upstream = branch.upstream().ok()?;
        upstream.name().ok().flatten().map(|s| s.to_string())
    }

    /// Stage a file
    pub fn stage_file(&self, path: &Path) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_path(path)?;
        index.write()?;
        Ok(())
    }

    /// Stage all files in a directory
    pub fn stage_directory(&self, path: &Path) -> Result<()> {
        // Use git add with the directory path
        let status = Command::new("git")
            .args(["add", &path.to_string_lossy()])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to stage directory")))
        }
    }

    /// Stage all changes
    pub fn stage_all(&self) -> Result<()> {
        let status = Command::new("git")
            .args(["add", "--all"])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to stage all")))
        }
    }

    /// Unstage a file
    pub fn unstage_file(&self, path: &Path) -> Result<()> {
        let status = Command::new("git")
            .args(["restore", "--staged", &path.to_string_lossy()])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to unstage file")))
        }
    }

    /// Unstage all files
    pub fn unstage_all(&self) -> Result<()> {
        let status = Command::new("git")
            .args(["restore", "--staged", "."])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to unstage all")))
        }
    }

    /// Discard changes to a file
    pub fn discard_changes(&self, path: &Path, is_untracked: bool) -> Result<()> {
        if is_untracked {
            // Delete untracked file
            std::fs::remove_file(path)?;
        } else {
            // Restore from HEAD
            let status = Command::new("git")
                .args([
                    "restore",
                    "--source=HEAD",
                    "--staged",
                    "--worktree",
                    &path.to_string_lossy(),
                ])
                .status()?;

            if !status.success() {
                return Err(FussrError::Git(git2::Error::from_str("Failed to discard changes")));
            }
        }
        Ok(())
    }

    /// Delete a file (tracked or untracked)
    pub fn delete_file(&self, path: &Path, is_untracked: bool) -> Result<()> {
        if is_untracked {
            std::fs::remove_file(path)?;
        } else {
            let status = Command::new("git")
                .args(["rm", "-f", &path.to_string_lossy()])
                .status()?;

            if !status.success() {
                return Err(FussrError::Git(git2::Error::from_str("Failed to delete file")));
            }
        }
        Ok(())
    }

    /// Create a commit with the given message (captures output to not corrupt TUI)
    pub fn commit(&self, message: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["commit", "-m", message])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to commit")))
        }
    }

    /// Amend the last commit (captures output to not corrupt TUI)
    pub fn commit_amend(&self, message: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["commit", "--amend", "-m", message])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to amend commit")))
        }
    }

    /// Get last commit message
    pub fn last_commit_message(&self) -> Option<String> {
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=%B"])
            .output()
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    /// Fetch from remote (captures output to not corrupt TUI)
    pub fn fetch(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["fetch"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.contains("Could not read from remote") {
                "Cannot reach remote - check connection/auth".to_string()
            } else if stderr.contains("does not appear to be a git repository") {
                "Remote not found. Add with: git remote add origin <url>".to_string()
            } else {
                "Fetch failed".to_string()
            };
            Err(FussrError::Git(git2::Error::from_str(&msg)))
        }
    }

    /// Fetch from a specific remote
    pub fn fetch_from_remote(&self, remote: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["fetch", remote])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.contains("Could not read from remote") {
                format!("Cannot reach '{}' - check connection/auth", remote)
            } else if stderr.contains("does not appear to be a git repository") {
                format!("Remote '{}' not found", remote)
            } else {
                format!("Fetch from '{}' failed", remote)
            };
            Err(FussrError::Git(git2::Error::from_str(&msg)))
        }
    }

    /// Pull from remote (captures output to not corrupt TUI)
    pub fn pull(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["pull"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.contains("no tracking information") || stderr.contains("no upstream") {
                format!("No upstream set. Run: git branch --set-upstream-to=origin/{}", self.branch_name())
            } else if stderr.contains("Could not read from remote") {
                "Cannot reach remote - check connection/auth".to_string()
            } else if stderr.contains("CONFLICT") || stderr.contains("Merge conflict") {
                "Pull has conflicts - resolve manually".to_string()
            } else if stderr.contains("not a git repository") {
                "Not in a git repository".to_string()
            } else {
                "Pull failed".to_string()
            };
            Err(FussrError::Git(git2::Error::from_str(&msg)))
        }
    }

    /// Push to remote (captures output to not corrupt TUI)
    pub fn push(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["push"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Detect common push errors and provide helpful messages
            let msg = if stderr.contains("no upstream branch") || stderr.contains("has no upstream") {
                format!("No upstream set. Run: git push -u origin {}", self.branch_name())
            } else if stderr.contains("does not appear to be a git repository") {
                "Remote not found. Check your remote config".to_string()
            } else if stderr.contains("rejected") {
                "Push rejected - pull first or force push".to_string()
            } else if stderr.contains("Could not read from remote") {
                "Cannot reach remote - check connection/auth".to_string()
            } else {
                "Push failed".to_string()
            };
            Err(FussrError::Git(git2::Error::from_str(&msg)))
        }
    }

    /// Check if current branch has an upstream configured
    pub fn has_upstream(&self) -> bool {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "@{upstream}"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        output.map(|o| o.status.success()).unwrap_or(false)
    }

    /// Get list of remote names
    pub fn get_remotes(&self) -> Vec<String> {
        let output = Command::new("git")
            .args(["remote"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Push with upstream set (git push -u <remote> <branch>)
    pub fn push_with_upstream(&self, remote: &str) -> Result<()> {
        let branch = self.branch_name();
        let output = Command::new("git")
            .args(["push", "-u", remote, &branch])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.contains("Could not read from remote") {
                format!("Cannot reach '{}' - check connection/auth", remote)
            } else if stderr.contains("does not appear to be a git repository") {
                format!("Remote '{}' not found", remote)
            } else if stderr.contains("rejected") {
                "Push rejected - pull first".to_string()
            } else {
                format!("Push to '{}' failed", remote)
            };
            Err(FussrError::Git(git2::Error::from_str(&msg)))
        }
    }

    /// Pull from a specific remote/branch (and set upstream)
    pub fn pull_from_remote(&self, remote: &str) -> Result<()> {
        let branch = self.branch_name();

        // First set upstream tracking
        let _ = Command::new("git")
            .args(["branch", "--set-upstream-to", &format!("{}/{}", remote, branch)])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        // Then pull
        let output = Command::new("git")
            .args(["pull", remote, &branch])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.contains("Could not read from remote") {
                format!("Cannot reach '{}' - check connection/auth", remote)
            } else if stderr.contains("CONFLICT") || stderr.contains("Merge conflict") {
                "Pull has conflicts - resolve manually".to_string()
            } else if stderr.contains("does not appear to be a git repository") {
                format!("Remote '{}' not found", remote)
            } else {
                format!("Pull from '{}' failed", remote)
            };
            Err(FussrError::Git(git2::Error::from_str(&msg)))
        }
    }

    /// Get diff for a file
    pub fn diff_file(&self, path: &Path, has_incoming: bool) -> Result<String> {
        let path_str = path.to_string_lossy();
        let output = if has_incoming {
            Command::new("git")
                .args(["diff", "HEAD...@{upstream}", "--", &path_str])
                .output()?
        } else {
            Command::new("git")
                .args(["diff", "HEAD", "--", &path_str])
                .output()?
        };

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Rename a file
    pub fn rename_file(&self, old_path: &Path, new_path: &Path) -> Result<()> {
        let status = Command::new("mv")
            .args(["-f", &old_path.to_string_lossy(), &new_path.to_string_lossy()])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(FussrError::Git(git2::Error::from_str("Failed to rename file")))
        }
    }
}
