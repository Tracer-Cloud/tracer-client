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
    println!("cargo:rerun-if-changed=c/");
    println!("cargo:rerun-if-changed=typegen/");

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
    std::fs::copy(
        "c/libbootstrap.a",
        Path::new(&out_dir).join("libbootstrap.a"),
    )
    .expect("failed to copy libbootstrap.a into OUT_DIR");
    std::fs::copy("c/.output/libbpf.a", Path::new(&out_dir).join("libbpf.a"))
        .expect("failed to copy libbpf.a into OUT_DIR");

    // ---------------------------------------------------------------------
    // 4. Link-time instructions for rustc
    // ---------------------------------------------------------------------

    // Tell cargo where to find the freshly built static libs
    println!("cargo:rustc-link-search=native={}", out_dir.display());

    // Link to the static libraries
    let bootstrap = out_dir.join("libbootstrap.a");
    let bpf = out_dir.join("libbpf.a");

    // Group required for cross-ref resolution
    println!("cargo:rustc-link-arg=-Wl,--start-group");
    println!("cargo:rustc-link-arg={}", bootstrap.display());
    println!("cargo:rustc-link-arg={}", bpf.display());
    println!("cargo:rustc-link-arg=-Wl,--end-group");

    // Link required system libraries: libbpf depends on libelf and zlib
    println!("cargo:rustc-link-arg=-lelf");
    println!("cargo:rustc-link-arg=-lz");

    // Link system libraries specific to AArch64: libgcc & libatomic
    println!("cargo:rustc-link-arg=-Wl,-Bstatic");
    println!("cargo:rustc-link-arg=-latomic");
    println!("cargo:rustc-link-arg=-lgcc");
    println!("cargo:rustc-link-arg=-Wl,-Bdynamic");
}
