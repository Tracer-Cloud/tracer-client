use std::env;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tell cargo to rerun this build script if any of the C files change
    println!("cargo:rerun-if-changed=c/");
    println!("cargo:rerun-if-changed=typegen/");

    // Get the output directory where we'll place the compiled library
    let out_dir = env::var("OUT_DIR")?;

    // Change directory to the C code directory and run make
    let status = Command::new("make").current_dir("c").arg("-j").status()?;

    if !status.success() {
        return Err("Failed to build C code with make".into());
    }

    // Copy the compiled libraries to the output directory
    std::fs::copy(
        "c/libbootstrap.a",
        Path::new(&out_dir).join("libbootstrap.a"),
    )?;

    // Copy libbpf.a
    std::fs::copy("c/.output/libbpf.a", Path::new(&out_dir).join("libbpf.a"))?;

    // Tell cargo where to find the libraries
    println!("cargo:rustc-link-search=native={}", out_dir);

    // Link to the static libraries
    println!("cargo:rustc-link-lib=static=bootstrap");
    println!("cargo:rustc-link-lib=static=bpf");

    // Link required system libraries
    println!("cargo:rustc-link-lib=elf");
    println!("cargo:rustc-link-lib=z");

    Ok(())
}
