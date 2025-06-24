fn main() {
    // useful to trigger changes in the cargo build on the json file
    println!("cargo:rerun-if-changed=src/process_identification/target_process/yml_rules/tracer.rules.yml");
    println!("cargo:rerun-if-changed=src/process_identification/target_process/yml_rules/tracer.exclude.yml");
    // write the build-time information to the file
    built::write_built_file().expect("Failed to acquire build-time information");
}
