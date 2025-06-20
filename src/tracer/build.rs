fn main() {
    // useful to trigger changes in the cargo build on the json file
    println!("cargo:rerun-if-changed=src/common/target_process/yaml_rules/default_rules.yml");
}
