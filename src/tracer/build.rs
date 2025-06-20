fn main() {
    // write the build-time information to the file
    built::write_built_file().expect("Failed to acquire build-time information");
    // useful to trigger changes in the cargo build on the json file
    println!("cargo:rerun-if-changed=src/common/target_process/default_rules.json");
}
