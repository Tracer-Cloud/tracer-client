use std::collections::HashSet;
use std::fs;
use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use lazy_static::lazy_static;
use predicates::prelude::predicate;
use predicates::str::RegexPredicate;
use predicates::Predicate;
use tempfile::TempDir;

use crate::utils::debug_log::Logger;
use crate::utils::upload::upload_from_file_path;

#[derive(Debug, Clone)]
pub struct WatchedFileInfo {
    pub path: String,
    pub size: u64,
    pub last_update: DateTime<Utc>,
    pub last_upload: Option<DateTime<Utc>>,
    pub cached_path: Option<String>,
    pub action: FileAction,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub directory: String,
    pub size: u64,
    pub last_update: DateTime<Utc>,
}

pub struct FileWatcher {
    watched_files: HashMap<String, WatchedFileInfo>,
    all_files: HashMap<String, FileInfo>,
    temp_dir: TempDir,
}

pub enum FilePattern {
    DirectoryPath(String),
    FilenameMatch(RegexPredicate),
    PathMatch(RegexPredicate),
}

#[derive(Clone, Debug)]
pub enum FileAction {
    None,
    Upload,
}

#[derive(Debug)]
enum FileUploadType {
    None,
    Old,
    New,
}

const CACHED_FILE_NAME_CHARSET: &str = "abcdefghijklmnoprstuwxyz0123456789";
const CACHED_FILE_NAME_LENGTH: usize = 16;

lazy_static! {
    static ref FILE_WATCHER_PATTERNS: Vec<(FilePattern, FileAction)> = vec![
        (
            FilePattern::FilenameMatch(predicate::str::is_match("P1s1Log.final.out").unwrap()),
            FileAction::Upload
        ),
        (
            FilePattern::FilenameMatch(predicate::str::is_match("Log.final.out").unwrap()),
            FileAction::Upload
        ),
        (
            FilePattern::FilenameMatch(predicate::str::is_match(".narrowPeak").unwrap()),
            FileAction::Upload
        ),
        (
            FilePattern::FilenameMatch(predicate::str::is_match("_counts.summary").unwrap()),
            FileAction::Upload
        ),
        (
            FilePattern::DirectoryPath("example-directory-path/".to_string()),
            FileAction::Upload
        ),
        (
            FilePattern::PathMatch(predicate::str::is_match("example-path[a-zA-Z]*").unwrap()),
            FileAction::Upload
        ),
        (
            FilePattern::FilenameMatch(predicate::str::is_match("example-filename").unwrap()),
            FileAction::Upload,
        ),
        (
            FilePattern::PathMatch(predicate::str::is_match("example-path-nonaction").unwrap()),
            FileAction::None
        ),
    ];
}

impl FileWatcher {
    pub fn with_random_temp_dir() -> Result<Self> {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        Ok(Self::new(temp_dir))
    }

    pub fn new(temp_dir: TempDir) -> Self {
        Self {
            watched_files: HashMap::new(),
            all_files: HashMap::new(),
            temp_dir,
        }
    }

    pub fn gather_all_files_from_directory(
        _all_files: &mut HashMap<String, FileInfo>,
        directory: &Path,
    ) {
        if !directory.exists() {
            println!("Warning: Directory does not exist: {}", directory.display());
        }

        // match directory.read_dir() {
        //     Ok(files) => {
        //         // Process files
        //         for file in files {
        //             match file {
        //                 Ok(file) => {
        //                     if file.path().is_dir() {
        //                         Self::gather_all_files_from_directory(all_files, &file.path());
        //                         continue;
        //                     }
        //
        //                     let file_path = file.path();
        //                     let file_path_string = match file_path.to_str() {
        //                         Some(path) => path,
        //                         None => {
        //                             println!(
        //                                 "Warning: Could not convert file path to string: {}",
        //                                 file_path.display()
        //                             );
        //                             continue;
        //                         }
        //                     };
        //
        //                     let directory = match file_path.parent() {
        //                         Some(parent) => match parent.to_str() {
        //                             Some(dir) => dir,
        //                             None => {
        //                                 println!("Warning: Could not convert parent directory to string: {}", parent.display());
        //                                 continue;
        //                             }
        //                         },
        //                         None => {
        //                             println!(
        //                                 "Warning: File has no parent directory: {}",
        //                                 file_path.display()
        //                             );
        //                             continue;
        //                         }
        //                     };
        //
        //                     let metadata = match file.metadata() {
        //                         Ok(meta) => meta,
        //                         Err(e) => {
        //                             println!(
        //                                 "Warning: Could not read metadata for file {}: {}",
        //                                 file_path.display(),
        //                                 e
        //                             );
        //                             continue;
        //                         }
        //                     };
        //
        //                     let last_update = match metadata.modified() {
        //                         Ok(time) => time.into(),
        //                         Err(e) => {
        //                             println!(
        //                                 "Warning: Could not get modification time for file {}: {}",
        //                                 file_path.display(),
        //                                 e
        //                             );
        //                             continue;
        //                         }
        //                     };
        //
        //                     let file_name = match file_path.file_name() {
        //                         Some(name) => {
        //                             match name.to_str() {
        //                                 Some(name_str) => name_str.to_string(),
        //                                 None => {
        //                                     println!("Warning: Could not convert file name to string: {}", file_path.display());
        //                                     continue;
        //                                 }
        //                             }
        //                         }
        //                         None => {
        //                             println!("Warning: File has no name: {}", file_path.display());
        //                             continue;
        //                         }
        //                     };
        //
        //                     let size = metadata.len();
        //
        //                     all_files.insert(
        //                         file_path_string.to_string(),
        //                         FileInfo {
        //                             name: file_name,
        //                             directory: directory.to_string(),
        //                             size,
        //                             last_update,
        //                         },
        //                     );
        //                 }
        //                 Err(e) => {
        //                     println!(
        //                         "Error reading directory entry in {}: {}",
        //                         directory.display(),
        //                         e
        //                     );
        //                 }
        //             }
        //         }
        //     }
        //     Err(e) => {
        //         println!("Error reading directory {}: {}", directory.display(), e);
        //
        //         // Handle common error cases with more specific messages
        //         if e.kind() == std::io::ErrorKind::PermissionDenied {
        //             println!(
        //                 "Permission denied when accessing directory: {}",
        //                 directory.display()
        //             );
        //         } else if e.kind() == std::io::ErrorKind::NotFound {
        //             println!("Directory not found: {}", directory.display());
        //         } else if e.raw_os_error() == Some(40) {
        //             // FilesystemLoop error code
        //             println!(
        //                 "Symbolic link loop detected in directory: {}",
        //                 directory.display()
        //             );
        //             println!("There are too many levels of symbolic links, possibly creating a circular reference.");
        //         }
        //     }
        // }
    }

    pub fn gather_pattern_from_directory(
        files: &HashMap<String, FileInfo>,
        current_watched_files: &mut HashMap<String, WatchedFileInfo>,
        pattern: &FilePattern,
        action: &FileAction,
    ) -> Result<()> {
        for (file_path, file_info) in files {
            let matched = match pattern {
                FilePattern::DirectoryPath(path) => file_info.directory == *path,
                FilePattern::FilenameMatch(regex) => regex.eval(&file_info.name),
                FilePattern::PathMatch(regex) => regex.eval(file_path),
            };

            if matched {
                current_watched_files.insert(
                    file_path.to_string(),
                    WatchedFileInfo {
                        path: file_path.to_string(),
                        size: file_info.size,
                        last_update: file_info.last_update,
                        cached_path: None,
                        action: action.clone(),
                        last_upload: None,
                    },
                );
            }
        }

        Ok(())
    }

    fn check_if_file_to_update<'a>(
        &self,
        old_file_info: Option<&'a WatchedFileInfo>,
        new_file_info: Option<&'a WatchedFileInfo>,
    ) -> bool {
        match (old_file_info, new_file_info) {
            (Some(old), Some(new)) => new.last_update > old.last_update,
            (None, Some(_)) => true,
            _ => false,
        }
    }

    fn check_if_file_to_upload<'a>(
        &self,
        new_size_duration: TimeDelta,
        old_file_info: Option<&'a WatchedFileInfo>,
        new_file_info: Option<&'a WatchedFileInfo>,
    ) -> FileUploadType {
        match (old_file_info, new_file_info) {
            (Some(old), Some(new)) => match (&old.action, &new.action) {
                (FileAction::Upload, FileAction::None) => {
                    if new.size < old.size {
                        FileUploadType::Old
                    } else {
                        FileUploadType::None
                    }
                }
                (FileAction::Upload, FileAction::Upload) => {
                    if new.last_update == old.last_update
                        && chrono::Utc::now() - new.last_update > new_size_duration
                        && (old.last_upload.is_none() || old.last_upload.unwrap() < new.last_update)
                    {
                        FileUploadType::New
                    } else if new.size < old.size {
                        FileUploadType::Old
                    } else {
                        FileUploadType::None
                    }
                }
                _ => FileUploadType::None,
            },
            (Some(old), None) => match &old.action {
                FileAction::Upload => FileUploadType::Old,
                _ => FileUploadType::None,
            },
            _ => FileUploadType::None,
        }
    }

    pub fn cache_file(&self, file_info: &mut WatchedFileInfo) -> Result<()> {
        if file_info.cached_path.is_none() {
            let file_name =
                random_string::generate(CACHED_FILE_NAME_LENGTH, CACHED_FILE_NAME_CHARSET);
            file_info.cached_path = Some(
                Path::new(self.temp_dir.path().as_os_str())
                    .join(file_name)
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Failed to convert cache path to string"))?
                    .to_string(),
            );
        }

        fs::copy(&file_info.path, file_info.cached_path.as_ref().unwrap()).with_context(|| {
            format!(
                "Failed to copy file from {} to cached location {}",
                file_info.path,
                file_info.cached_path.as_ref().unwrap()
            )
        })?;

        Ok(())
    }

    pub async fn upload_file(
        &self,
        service_url: &str,
        api_key: &str,
        file_info: &WatchedFileInfo,
    ) -> Result<()> {
        let logger = Logger::new();
        logger
            .log(&format!("Uploading file: {}", file_info.path), None)
            .await;

        let file_path = file_info.cached_path.as_ref().unwrap_or(&file_info.path);

        let file_name = Path::new(&file_info.path)
            .file_name()
            .ok_or_else(|| {
                anyhow::anyhow!("Failed to get file name from path: {}", file_info.path)
            })?
            .to_str();

        upload_from_file_path(service_url, api_key, file_path, file_name)
            .await
            .with_context(|| format!("Failed to upload file: {}", file_path))?;

        Ok(())
    }

    pub fn get_file_by_path_suffix(&self, path_suffix: &str) -> Option<(&String, &FileInfo)> {
        let path = self.all_files.keys().find(|path| {
            path.ends_with(path_suffix)
                && match path.split('/').next_back() {
                    Some(last) => path_suffix.contains(last),
                    None => {
                        println!("Warning: Could not determine filename from path: {}", path);
                        false
                    }
                }
        });

        path.and_then(|p| self.all_files.get(p).map(|info| (p, info)))
    }

    pub async fn poll_files(
        &mut self,
        service_url: &str,
        api_key: &str,
        workflow_directory: &str,
        file_cache_dir: &str,
        new_size_duration: TimeDelta,
    ) -> Result<()> {
        let logger = Logger::new();
        let mut to_upload: Vec<WatchedFileInfo> = Vec::new();
        let workflow_path = Path::new(workflow_directory);
        if !workflow_path.exists() {
            logger
                .log(
                    &format!(
                        "Workflow directory does not exist: {}",
                        workflow_path.display()
                    ),
                    None,
                )
                .await;
            return Err(anyhow::anyhow!(
                "Workflow directory does not exist: {}",
                workflow_path.display()
            ));
        }

        let mut found_files = HashMap::new();
        Self::gather_all_files_from_directory(&mut found_files, workflow_path);

        let mut watched_files = self.watched_files.clone();

        for (pattern, action) in FILE_WATCHER_PATTERNS.iter() {
            Self::gather_pattern_from_directory(&found_files, &mut watched_files, pattern, action)
                .with_context(|| "Failed to gather files matching pattern")?;
        }

        let paths = found_files.keys().cloned().collect::<Vec<String>>();

        logger
            .log(&format!("Found {} files", paths.len()), None)
            .await;
        if paths.is_empty() {
            logger
                .log(
                    "Warning: No files were found in the workflow directory",
                    None,
                )
                .await;
        }

        let paths: HashSet<String> = HashSet::from_iter(
            [
                paths,
                self.watched_files.keys().cloned().collect::<Vec<String>>(),
            ]
            .concat(),
        );

        // Upload action processing
        for path in paths {
            let old_file_info = self.watched_files.get(&path);
            let new_file_info = watched_files.get_mut(&path);

            let upload_type = self.check_if_file_to_upload(
                new_size_duration,
                old_file_info,
                new_file_info.as_deref(),
            );

            match upload_type {
                FileUploadType::Old => {
                    if let Some(old_info) = old_file_info {
                        to_upload.push(old_info.clone());
                    }
                }
                FileUploadType::New => {
                    if let Some(new_file_info) = new_file_info {
                        new_file_info.last_upload = Some(Utc::now());
                        to_upload.push(new_file_info.clone());
                    }
                }
                _ => {}
            }
        }

        for file_info in to_upload {
            self.upload_file(service_url, api_key, &file_info)
                .await
                .with_context(|| format!("Failed to upload file: {}", file_info.path))?;
        }

        for file_info in watched_files.values_mut() {
            let old_file_info = self.watched_files.get(&file_info.path);
            let update = self.check_if_file_to_update(old_file_info, Some(file_info));
            if update {
                self.cache_file(file_info)
                    .with_context(|| format!("Failed to cache file: {}", file_info.path))?;
            } else if let Some(old_file_info) = old_file_info {
                file_info.cached_path = old_file_info.cached_path.clone();
                file_info.last_upload = old_file_info.last_upload;
            }
        }

        self.watched_files = watched_files;
        self.all_files = found_files;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Days;

    use super::*;

    #[test]
    fn test_check_if_file_to_update_no_changes() {
        let now: DateTime<Utc> = Utc::now();
        let file_watcher = FileWatcher::with_random_temp_dir().unwrap();
        let old_file_info = WatchedFileInfo {
            path: "/tmp/test.txt".to_string(),
            size: 50,
            last_update: now,
            last_upload: Some(now),
            cached_path: None,
            action: FileAction::None,
        };

        let new_file_info = WatchedFileInfo {
            path: "/tmp/test.txt".to_string(),
            size: 50,
            last_update: now,
            last_upload: Some(now),
            cached_path: None,
            action: FileAction::None,
        };

        assert!(!file_watcher.check_if_file_to_update(Some(&old_file_info), Some(&new_file_info)));
    }

    #[test]
    fn test_check_if_file_to_update_new_file() {
        let now: DateTime<Utc> = Utc::now();
        let file_watcher = FileWatcher::with_random_temp_dir().unwrap();
        let old_file_info = WatchedFileInfo {
            path: "/tmp/test.txt".to_string(),
            size: 50,
            last_update: now,
            last_upload: Some(now),
            cached_path: None,
            action: FileAction::None,
        };

        let newer = now.checked_add_days(Days::new(1)).unwrap();
        let new_file_info = WatchedFileInfo {
            path: "/tmp/test.txt".to_string(),
            size: 50,
            last_update: newer,
            last_upload: Some(now),
            cached_path: None,
            action: FileAction::None,
        };

        assert!(file_watcher.check_if_file_to_update(Some(&old_file_info), Some(&new_file_info)));
    }
}
