use crate::types::TracerVersion;
use colored::Colorize;
use std::io;
use std::path::{Component, Path, PathBuf};

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

/// Strict path sanitizer: returns a path *beneath* `base_dir`.
pub fn sanitize_path(base_dir: &Path, subdir: &str) -> io::Result<PathBuf> {
    // SAFETY: we sanitize this path to ensure it is relative, non-empty, and does not contain
    // any disallowed path components
    let subdir_path = PathBuf::from(subdir); // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path

    // 1) Must be relative
    if subdir_path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "absolute paths not allowed",
        ));
    }

    // 2) Reject empty / NUL / sneaky components
    if subdir_path.as_os_str().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty path"));
    }
    for c in subdir_path.components() {
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

    // 3) Build a candidate path and canonicalize both sides
    // NOTE: canonicalize follows symlinks; that’s OK if we enforce "beneath base" after.
    let base_real = base_dir.canonicalize()?;
    let candidate = base_real.join(subdir_path);
    let candidate_real = candidate.canonicalize()?;

    // 4) Enforce "beneath base"
    if !candidate_real.starts_with(&base_real) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "path escapes base",
        ));
    }

    Ok(candidate_real)
}
