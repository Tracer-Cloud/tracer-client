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

    let version_code = kernel_version_code();
    println!("cargo:warning=Detected linux version code {}", version_code);

    // version code for 5.5:    KERNEL_VERSION(5,5,0) = 328192
    let min_version_code = (5 * 65536) + (5 * 256); // 328192

    if version_code < min_version_code {
        println!(
            "cargo:warning=Skipping eBPF build: kernel headers < 5.5 (version_code={})",
            version_code
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


fn kernel_version_code() -> u32 {
    let header = std::fs::read_to_string("/usr/include/linux/version.h")
        .unwrap_or_default();

    // Look for: #define LINUX_VERSION_CODE 331264
    for line in header.lines() {
        if let Some(rest) = line.strip_prefix("#define LINUX_VERSION_CODE") {
            return rest.trim().parse::<u32>().unwrap_or(0);
        }
    }
    0
}
