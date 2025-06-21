fn main() {
    // useful to trigger changes in the cargo build on the json file
    println!("cargo:rerun-if-changed=src/common/target_process/yml_rules/tracer.rules.yml");
    println!("cargo:rerun-if-changed=src/common/target_process/yml_rules/tracer.exclude.yml");
}
