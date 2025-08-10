use crate::types::TracerVersion;
use colored::Colorize;
use std::io;
use std::path::{Component, Path, PathBuf};
use tempfile::TempDir;

pub enum TagColor {
    Green,
    Red,
    Blue,
    Cyan,
}

pub fn print_message(tag: &str, message: &str, color: TagColor) {
    let tag = format!("[{tag}]");
    let tag = match color {
        TagColor::Green => tag.green(),
        TagColor::Red => tag.red(),
        TagColor::Blue => tag.blue(),
        TagColor::Cyan => tag.cyan(),
    }
    .bold();
    const PADDING: usize = 9;
    let padded = format!("{tag:>width$}", width = PADDING);
    println!("{padded} {message}");
}

pub fn print_status(tag: &str, label: &str, reason: &str, color: TagColor) {
    const PADDING: usize = 30;

    let label = if !reason.is_empty() {
        format!("{}:", label)
    } else {
        label.to_string()
    };
    let padded = format!("{label:<width$}", width = PADDING);
    print_message(tag, format!("{padded}{reason}").as_str(), color);
}

pub fn _print_anteater_banner_v2(version: &TracerVersion) {
    println!("                    ___,,___");
    println!("               _,-='=- =-  -`\"--.__,,.._");
    println!("            ,-;// /  - -       -   -= - \"=.");
    println!("          ,'///    -     -   -   =  - ==-=\\`.");
    println!("         |/// /  =    `. - =   == - =.=_,,._ `=/|");
    println!("        ///    -   -    \\  - - = ,ndDMHHMM/\\b  \\\\");
    println!("      ,' - / /        / /\\ =  - /MM(,,._`YQMML  `|");
    println!("     <_,=^Kkm / / / / ///H|wnWWdMKKK#\"\"-;. `\"0\\  |");
    println!("            `\"\"QkmmmmmnWMMM\\\"\"WHMKKMM\\   `--. \\> \\");
    println!("     hjm          `\"\"'  `->>>    ``WHMb,.    `-_<@)");
    println!("                                    `\"QMM`.");
    println!("                                       `>>>");
    println!("{} ", "Tracer Installer".yellow().bold());
    println!(
        "{} {}",
        "Tracer version:".bold(),
        version.to_string().cyan().bold()
    );
}

pub fn print_anteater_banner(version: &TracerVersion) {
    println!(" ");
    println!("⠀⠀⠀⠀⠀⠀⠀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ ");
    println!(
        "⠀⢷⣦⣦⣄⣄⣔⣿⣿⣆⣄⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀│ {}",
        "Tracer.bio CLI Installer".bold()
    );
    println!("⠀⠀⠻⣿⣿⣿⣿⣿⣿⣿⣿⠛⣿⣷⣦⡄⡀⠀⠀⠀⠀⠀⠀⠀⠀│ ");
    println!("⠀⠀⠀⠈⠻⣻⣿⣿⣿⣿⣿⣷⣷⣿⣿⣿⣷⣧⡄⡀⠀⠀⠀⠀⠀│ ");
    println!(
        "⠀⠀⠀⠀⠀⠀⠘⠉⠃⠑⠁⠃⠋⠋⠛⠟⢿⢿⣿⣷⣦⡀⠀⠀⠀│ Tracer version: {}",
        version.to_string().blue().bold()
    );
    println!("⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠑⠙⠻⠿⣧⠄⠀│ ");
    println!("⠀          ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠀⠀│ ");
    println!(" ");
}

pub fn print_title(title: &str) {
    println!("\n==== {} ====\n", title.bold());
}

pub trait TrustedDir {
    fn get_trusted_path(&self) -> io::Result<PathBuf>;

    fn get_trusted_subpath(&self, subdir: SanitizedRelativePath) -> io::Result<PathBuf> {
        // Build a candidate path and canonicalize both sides
        // NOTE: canonicalize follows symlinks; that’s OK if we enforce "beneath base" after.
        let base = self.get_trusted_path()?;
        let candidate = base.join(subdir.into_path()).canonicalize()?;

        // 4) Enforce "beneath base"
        if !candidate.starts_with(&base) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path escapes base",
            ));
        }

        Ok(candidate)
    }
}

impl TrustedDir for TempDir {
    fn get_trusted_path(&self) -> io::Result<PathBuf> {
        self.path().canonicalize()
    }
}

pub struct SanitizedRelativePath(PathBuf);

impl SanitizedRelativePath {
    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

impl TryFrom<&str> for SanitizedRelativePath {
    type Error = io::Error;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        // SAFETY: we sanitize this path to make sure it is relative and does not contain any
        // unsafe components (e.g. '..')
        let path = PathBuf::from(path); // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
        check_sanitary_relative_path(&path)?;
        Ok(Self(path))
    }
}

impl TryFrom<PathBuf> for SanitizedRelativePath {
    type Error = io::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        check_sanitary_relative_path(&path)?;
        Ok(Self(path))
    }
}

pub fn check_sanitary_relative_path(path: &Path) -> io::Result<()> {
    // 1) Must be relative
    if path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "absolute paths not allowed",
        ));
    }

    // 2) Reject empty / NUL / sneaky components
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty path"));
    }

    for c in path.components() {
        match c {
            Component::Normal(_) => {}
            // reject ., .., prefix (Windows), or root components
            Component::CurDir
            | Component::ParentDir
            | Component::Prefix(_)
            | Component::RootDir => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "invalid component",
                ))
            }
        }
    }

    Ok(())
}
