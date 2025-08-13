# Security Roadmap

## Vulnerability Identification

We use multiple approaches to identify vulnerabilities in our codebase, including:

* Static analysis tools (semgrep)
* Consultation with security experts
* Policy encouraging user reporting (see [SECURITY.md](../SECURITY.md))

## Vulnerability Mitigation

We take all security vulnerabilities seriously and will work to address them as quickly as possible. Our current process is:

1. The team triages the issue privately.
2. If valid, a GitHub Security Advisory will be created.
3. If the issue is valid and urgent, we prioritize fixing it immediately.
4. If the issue is valid and not urgent, we:
    * Implement any reasonable mitigations to reduce risk
    * Add any more time-consuming mitigations to the roadmap below
    * If necessary and appropriate, add a semgrep exception and document the reasoning below
5. Upon fixing any security issues, a new release is issued with a public disclosure of the issues and mitigations.

## Existing Security Issues and Mitigations

1. The `tracer` client spawns a child daemon process using the same binary. This triggers the `rust.lang.security.current-exe.current-exe` semgrep rule. There are no completely secure ways to determine the path of the current executable. We currently use best practices to spawn the client in the most secure way possible on the platform where the binary is running:
   1. On Linux
      1. We use `/proc/self/exe` to get a file descriptor for the current executable.
      2. We use `fork` to create the child process, and then `execveat` or `fexecve` to execute the command.
         1. This requires using `unsafe`, which requires another semgrep exception.
      3. If neither of these are available, we fall back to using `std::process::Command` with `/proc/self/fd/<fd>`.
   2. On other platforms (or if none of the Linux-specific methods work on a Linux platform) we use `std::process::Command` to spawn the child process.
      1. We use `std::env::current_exe` to get the path of the current executable at startup and verify it again when spawing the child process.
      2. We verify that the inode of the current exe has not changed between startup and spawning (on platforms where inode is available).
      3. We verify that the current exe matches `argv[0]`.
      4. We verify that the current exe path has no world-writable components.

2. The tracer insaller constructs a URL to download the tracer binary from. This triggers the `rust.actix.ssrf.reqwest-taint.reqwest-taint` semgrep rule.
   * The URL is constructed from a static base URL and a filename based on the platform and architecture. No user input is involved.
   * The URL is then parsed and validated before downloading.

3. After downloading the tracer binary tarball, the installer extracts it to a temporary directory, then moves the extracted binary to the final installation directory, which is hardcoded to `/usr/local/bin`. This triggers the `rust.actix.path-traversal.tainted-path.tainted-path ` semgrep rule.
   * The temporary directory is created using `tempfile::TempDir`.
   * The final installation directory is hardcoded and not user-provided.
   * None of the paths are constructed from user input.

## Security Roadmap

1. Implement code signing and verification for all binaries on all platforms (Q4 2025)
2. Implement SSRF protection for all HTTP requests (Q4 2025)