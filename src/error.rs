use thiserror::Error;

#[derive(Error, Debug)]
pub enum FussrError {
    #[error("Not a git repository")]
    NotAGitRepo,

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No files to display")]
    NoFiles,

    #[error("Terminal error: {0}")]
    Terminal(String),
}

pub type Result<T> = std::result::Result<T, FussrError>;
