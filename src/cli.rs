use clap::Parser;

/// fussr - A git staging TUI tool
///
/// Navigate and stage files with a tree view interface.
/// Rust port of fuss (written in Fortran).
#[derive(Parser, Debug)]
#[command(name = "fussr")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Show all files, not just dirty ones
    #[arg(short = 'a', long = "all")]
    pub show_all: bool,

    /// Show full file paths (alias for --all)
    #[arg(short = 'f', long = "full")]
    pub full: bool,

    /// Print tree and exit (non-interactive mode)
    #[arg(short = 'p', long = "print")]
    pub print_only: bool,

    /// Interactive mode (default behavior)
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,
}

impl Args {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Whether to show all files
    pub fn show_all_files(&self) -> bool {
        self.show_all || self.full
    }

    /// Whether to run in interactive mode
    pub fn is_interactive(&self) -> bool {
        !self.print_only
    }
}
