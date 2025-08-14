use crate::info_message;
use colored::Colorize;
use std::path::PathBuf;

pub struct OtelFileScanner;

impl OtelFileScanner {
    pub fn scan_watch_directory(
        watch_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let watch_dir = watch_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        info_message!("Directory being watched: {}", watch_dir.display());

        let patterns = Self::get_watch_patterns();
        info_message!("Watching for files matching these patterns:");
        for pattern in &patterns {
            info_message!("  - {}", pattern);
        }

        info_message!("Existing files that match patterns:");
        let mut found_files = false;

        if watch_dir.exists() {
            info_message!("Searching in: {}", watch_dir.display());
            let log_files = Self::find_log_files(&watch_dir, 0, 5);
            for path in log_files {
                info_message!("    {}", path.display());
                found_files = true;
            }
        }

        if !found_files {
            info_message!("No existing log files found - collector will watch for new files");
        }

        Ok(())
    }

    fn get_watch_patterns() -> Vec<&'static str> {
        vec![
            "**/.nextflow.log*",
            "**/nextflow.log*",
            "**/.command.log",
            "**/.command.err",
            "**/.command.out",
        ]
    }

    fn find_log_files(
        dir: &std::path::Path,
        depth: usize,
        max_depth: usize,
    ) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        if depth > max_depth {
            return files;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if Self::should_skip_directory(&name_str) {
                        continue;
                    }
                }

                if path.is_file() {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    if Self::is_log_file(&file_name) {
                        files.push(path);
                    }
                } else if path.is_dir() && depth < max_depth {
                    files.extend(Self::find_log_files(&path, depth + 1, max_depth));
                }
            }
        }
        files
    }

    fn should_skip_directory(name: &str) -> bool {
        let skip_dirs = [
            ".",
            "Library",
            "node_modules",
            "target",
            "build",
            "vendor",
            ".git",
            ".cargo",
            ".rustup",
            ".local",
            ".cache",
            "tmp",
            "var",
            "Applications",
            "Movies",
            "Music",
            "Pictures",
            "Public",
            "miniconda3",
        ];

        skip_dirs
            .iter()
            .any(|&skip_dir| name == skip_dir || name.starts_with('.'))
    }

    fn is_log_file(file_name: &str) -> bool {
        file_name.contains("nextflow")
            || file_name == ".command.log"
            || file_name == ".command.err"
            || file_name == ".command.out"
    }
}
