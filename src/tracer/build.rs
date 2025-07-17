use std::fs;


fn trigger_yml_files_from_dir(dir_path: &str) {
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "yml" || extension == "yaml" {
                    if let Some(path_str) = path.to_str() {
                        println!("cargo:rerun-if-changed={}", path_str);
                    }
                }
            }
        }
    }
}

fn main() {
    // useful to trigger changes in the cargo build on the yml files
    trigger_yml_files_from_dir("src/process_identification/target_pipeline/yml_rules");
    trigger_yml_files_from_dir("src/process_identification/target_process/yml_rules");

    // write the build-time information to the file
    built::write_built_file().expect("Failed to acquire build-time information");
}
