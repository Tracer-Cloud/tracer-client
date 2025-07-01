use std::fs;

fn main() {
    // useful to trigger changes in the cargo build on the json file
    let pipeline_rules_dir = "src/process_identification/target_pipeline/yml_rules";

    if let Ok(entries) = fs::read_dir(pipeline_rules_dir) {
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

    let process_rules_dir = "src/process_identification/target_process/yml_rules";

    if let Ok(entries) = fs::read_dir(process_rules_dir) {
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

    // write the build-time information to the file
    built::write_built_file().expect("Failed to acquire build-time information");
}
