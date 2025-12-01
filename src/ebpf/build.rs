use std::env;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Skip build on macOS and Windows since eBPF is Linux-specific
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    println!("target OS: {}", target_os.as_str());

    if target_os == "macos" || target_os == "windows" {
        println!(
            "cargo:warning=Skipping eBPF build on {} (Linux only)",
            target_os
        );
        return Ok(());
    }

    let kernel = String::from_utf8(
        Command::new("uname")
            .arg("-r")
            .output()?
            .stdout,
    )?
    .trim()
    .to_string();

    println!("cargo:warning=Detected kernel {}", kernel);

    // Parse major.minor
    let mut parts = kernel.split('.');

    let major = parts.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
    let minor = parts.next().unwrap_or("0").parse::<u32>().unwrap_or(0);

    // libbpf CO-RE requires kernel headers ≥ 5.5
    if major < 5 || (major == 5 && minor < 5) {
        println!(
            "cargo:warning=Skipping eBPF build: kernel {} < 5.5",
            kernel
        );
        return Ok(());
    }

    // Tell cargo to rerun this build script if any of the C files change
    println!("cargo:rerun-if-changed=c/");

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
