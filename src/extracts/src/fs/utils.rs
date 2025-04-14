use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct LogSearchConfig {
    /// Directories to search in priority order
    search_dirs: Vec<PathBuf>,

    /// Directory patterns to always skip
    skip_patterns: Vec<String>,

    /// Maximum directory depth to search (inclusive).
    ///
    /// Note: For finding files at depth N, set this to N+1 because:
    /// - A directory at depth N will be yielded
    /// - Its contents (files) will be at depth N+1
    /// - WalkDir only traverses if max_depth > current depth
    ///
    /// Example:
    /// - To find /a/b/c/file.log (depth=3):
    ///   Set max_depth=4
    max_depth: usize,
}

#[derive(Debug, Clone)]
pub struct FileFinder {
    config: LogSearchConfig,
}

impl FileFinder {
    pub fn new(config: LogSearchConfig) -> Self {
        Self { config }
    }

    pub fn should_skip_dir(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        self.config
            .skip_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern))
    }

    pub fn search_dir(&self, base_dir: &Path) -> Option<PathBuf> {
        WalkDir::new(base_dir)
            .max_depth(self.config.max_depth)
            .follow_links(true)
            .into_iter()
            .filter_entry(|entry| !self.should_skip_dir(entry.path()))
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name() == ".nextflow.log")
            .map(|e| e.into_path())
    }

    pub fn try_find(&self) -> Option<PathBuf> {
        self.config
            .search_dirs
            .iter()
            .find_map(|base_dir| self.search_dir(base_dir))
    }
}

impl Default for LogSearchConfig {
    fn default() -> Self {
        Self {
            search_dirs: vec![
                std::env::current_dir().unwrap_or_default(),
                dirs::home_dir().unwrap_or_default(),
                PathBuf::from(".nextflow"),
                PathBuf::from(".config/nextflow"),
                PathBuf::from("/"), // Fallback
            ],
            skip_patterns: vec![
                "/proc/".into(),
                "/sys/".into(),
                "/dev/".into(),
                "/.git/".into(),
                "/node_modules/".into(),
                "/target/".into(),
                "/.cache/".into(),
            ],
            max_depth: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::{tempdir, TempDir};

    // Helper to create test directory structure
    fn create_test_env() -> (TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join(".nextflow.log");
        File::create(&log_path).unwrap();

        // Create some subdirectories
        fs::create_dir_all(dir.path().join("subdir")).unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();

        (dir, log_path)
    }

    #[test]
    fn test_file_finder_finds_log_in_root() {
        let (dir, expected_path) = create_test_env();
        let config = LogSearchConfig {
            search_dirs: vec![dir.path().to_path_buf()],
            ..Default::default()
        };

        let finder = FileFinder::new(config);
        let found_path = finder.try_find();
        assert_eq!(found_path, Some(expected_path));
    }

    #[test]
    fn test_file_finder_finds_log_in_subdir() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("workdir");
        fs::create_dir(&subdir).unwrap();
        let log_path = subdir.join(".nextflow.log");
        File::create(&log_path).unwrap();

        let config = LogSearchConfig {
            search_dirs: vec![dir.path().to_path_buf()],
            max_depth: 2,
            ..Default::default()
        };

        let finder = FileFinder::new(config);
        let found_path = finder.try_find();
        assert_eq!(found_path, Some(log_path));
    }

    #[test]
    fn test_file_finder_skips_ignored_directories() {
        let dir = tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();
        File::create(git_dir.join(".nextflow.log")).unwrap();

        let config = LogSearchConfig {
            search_dirs: vec![dir.path().to_path_buf()],
            skip_patterns: vec!["/.git/".into()],
            ..Default::default()
        };

        let finder = FileFinder::new(config);
        let found_path = finder.try_find();
        //let found_path = tokio_test::block_on(finder.try_find()).unwrap();
        assert!(found_path.is_none());
    }

    #[test]
    fn test_file_finder_respects_max_depth() {
        let dir = tempdir().unwrap(); // depth 0
        let deep_dir = dir.path().join("a/b/c/d/");
        fs::create_dir_all(&deep_dir).unwrap();
        let log_path = deep_dir.join(".nextflow.log");
        File::create(&log_path).unwrap();

        // Shouldn't find it with depth 3
        let shallow_config = LogSearchConfig {
            search_dirs: vec![dir.path().to_path_buf()],
            max_depth: 3,
            ..Default::default()
        };

        let finder = FileFinder::new(shallow_config);
        let found_path = finder.try_find();
        assert!(found_path.is_none());

        // Should find it with depth 5
        let deep_config = LogSearchConfig {
            search_dirs: vec![dir.path().to_path_buf()],
            max_depth: 5,
            ..Default::default()
        };

        let finder = FileFinder::new(deep_config);
        let found_path = finder.try_find();
        assert_eq!(found_path, Some(log_path));
    }

    #[test]
    fn test_file_finder_checks_multiple_directories() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        let log_path = dir2.path().join(".nextflow.log");
        File::create(&log_path).unwrap();

        let config = LogSearchConfig {
            search_dirs: vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()],
            ..Default::default()
        };

        let finder = FileFinder::new(config);
        let found_path = finder.try_find();
        assert_eq!(found_path, Some(log_path));
    }

    #[test]
    fn test_file_finder_handles_nonexistent_directories() {
        let config = LogSearchConfig {
            search_dirs: vec![PathBuf::from("/nonexistent/path")],
            ..Default::default()
        };

        let finder = FileFinder::new(config);
        let result = finder.try_find();
        assert!(result.is_none());
    }
}
