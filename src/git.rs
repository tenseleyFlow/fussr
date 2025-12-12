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
            Err(FussrError::Git(git2::Error::from_str("Failed to fetch")))
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
            Err(FussrError::Git(git2::Error::from_str("Failed to pull")))
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
            Err(FussrError::Git(git2::Error::from_str("Failed to push")))
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
