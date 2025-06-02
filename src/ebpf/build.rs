use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    // ---------------------------------------------------------------------
    // 1. Skip non-Linux targets early – we still want the crate to compile
    //    cleanly on macOS/Windows for dev-convenience.
    // ---------------------------------------------------------------------
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "linux" {
        println!("cargo:warning=eBPF support disabled on non-Linux target");
        return;
    }

    // ---------------------------------------------------------------------
    // 2. Tell Cargo when to re-run this script
    // ---------------------------------------------------------------------
    println!("cargo:rerun-if-changed=c/Makefile");
    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-changed=typegen");

    // ---------------------------------------------------------------------
    // 3. Build the C/C++ + eBPF world with `make`
    // ---------------------------------------------------------------------
    let make_status = Command::new("make")
        .current_dir("c")
        .arg("-j")
        .status()
        .expect("failed to spawn make");

    if !make_status.success() {
        panic!("c/Makefile failed with status: {make_status}");
    }

    // Copy the compiled libraries to the output directory
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let src_bootstrap = Path::new("c").join("libbootstrap.a");
    let dst_bootstrap = out_dir.join("libbootstrap.a");
    std::fs::copy(&src_bootstrap, &dst_bootstrap)
        .expect("failed to copy libbootstrap.a into OUT_DIR");
    std::fs::copy("c/.output/libbpf.a", Path::new(&out_dir).join("libbpf.a"))
        .expect("failed to copy libbpf.a into OUT_DIR");

    // ---------------------------------------------------------------------
    // 4. Link-time hints for rustc
    // ---------------------------------------------------------------------
    // Tell rustc where the freshly built static libs live
    println!("cargo:rustc-link-search=native={}", out_dir.display());

    // Link order matters: first our own C++ wrapper, then libbpf
    println!("cargo:rustc-link-lib=static=bootstrap");
    println!("cargo:rustc-link-lib=static=bpf");

    // libbootstrap.a is C++, so we need libstdc++
    println!("cargo:rustc-link-lib=dylib=stdc++");
    // println!("cargo:rustc-link-lib=static=supc++");

    // libbpf depends on libelf and zlib
    println!("cargo:rustc-link-lib=elf");
    println!("cargo:rustc-link-lib=z");
}

// use std::env;
// use std::path::Path;
// use std::process::Command;

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     // Skip build on macOS and Windows since eBPF is Linux-specific
//     let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
//     println!("target OS: {}", target_os.as_str());

//     if target_os == "macos" || target_os == "windows" {
//         println!(
//             "cargo:warning=Skipping eBPF build on {} (Linux only)",
//             target_os
//         );
//         return Ok(());
//     }

//     // Tell cargo to rerun this build script if any of the C files change
//     println!("cargo:rerun-if-changed=c/");
//     println!("cargo:rerun-if-changed=typegen/");

//     // Get the output directory where we'll place the compiled library
//     let out_dir = env::var("OUT_DIR")?;

//     // Build C library
//     Command::new("make").current_dir("c").arg("-j").status()?;

//     // Copy the compiled libraries to the output directory
//     std::fs::copy(
//         "c/libbootstrap.a",
//         Path::new(&out_dir).join("libbootstrap.a"),
//     )?;

//     // Copy libbpf.a
//     std::fs::copy("c/.output/libbpf.a", Path::new(&out_dir).join("libbpf.a"))?;

//     // Tell cargo where to find the libraries
//     println!("cargo:rustc-link-search=native={}", out_dir);

//     // Link to the static libraries
//     println!("cargo:rustc-link-lib=static=bootstrap");
//     println!("cargo:rustc-link-lib=static=bpf");
//     println!("cargo:rustc-link-lib=dylib=stdc++");

//     // Link required system libraries
//     println!("cargo:rustc-link-lib=elf");
//     println!("cargo:rustc-link-lib=z");

//     Ok(())
// }
