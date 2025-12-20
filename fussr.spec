Name:           fussr
Version:        0.2.13
Release:        1%{?dist}
Summary:        A git staging TUI tool - Rust port of fuss

License:        MIT
URL:            https://github.com/tenseleyFlow/fussr
Source0:        %{name}-%{version}.tar.gz

BuildArch:      x86_64
BuildRequires:  rust >= 1.70
BuildRequires:  cargo

# Disable debug package
%global debug_package %{nil}

Requires:       git

%description
FUSSR is a git staging TUI tool, a Rust port of the fuss utility.
It provides an interactive terminal interface for staging and managing
git changes with a tree-based file view.

Features:
- Interactive tree view of git repository changes
- Easy staging/unstaging of files and hunks
- Keyboard-driven navigation
- Support for git operations (commit, push, pull, fetch, tag)
- Clean TUI interface built with ratatui

%prep
%autosetup

%build
export CARGO_TARGET_DIR=target
cargo build --release

%install
install -Dm755 target/release/%{name} %{buildroot}%{_bindir}/%{name}

%files
%{_bindir}/%{name}

%changelog
* Fri Dec 13 2024 mfw <espadon@outlook.com> - 0.1.0-1
- Initial RPM release of fussr
- Git staging TUI tool - Rust port of fuss
- Interactive tree view with staging support
- Full git operations suite
