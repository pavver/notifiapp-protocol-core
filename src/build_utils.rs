use std::process::Command;

/// Standard logic for generating protocol name and version at build time.
/// This should be called from the final protocol crate's `build.rs`.
pub fn configure_protocol_build() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    // Get protocol name from git root directory name
    let toplevel_output = Command::new("git")
        .current_dir(&manifest_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("Failed to execute git rev-parse");

    if !toplevel_output.status.success() {
        panic!("git rev-parse failed (are you inside a git repository?)");
    }

    let toplevel_path = String::from_utf8(toplevel_output.stdout).unwrap();
    let toplevel_path = toplevel_path.trim();
    let repo_name = std::path::Path::new(toplevel_path)
        .file_name()
        .expect("Failed to get repo name")
        .to_str()
        .unwrap()
        .to_string();

    // Get version from git hash and date
    // Format: {hash}_12.02.2022T12-22-32
    let show_output = Command::new("git")
        .current_dir(&manifest_dir)
        .args([
            "show",
            "-s",
            "--format=%h_%cd",
            "--date=format-local:%d.%m.%YT%H-%M-%S",
            "HEAD",
        ])
        .output()
        .expect("Failed to execute git show");

    if !show_output.status.success() {
        panic!("git show failed");
    }

    let version = String::from_utf8(show_output.stdout).unwrap();
    let version = version.trim().to_string();

    // Validate characters
    validate_protocol_string(&repo_name);
    validate_protocol_string(&version);

    println!("cargo:rustc-env=PROTOCOL_NAME={}", repo_name);
    println!("cargo:rustc-env=PROTOCOL_VERSION={}", version);

    // Rerun ONLY when commits change, not files
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
}

pub fn validate_protocol_string(s: &str) {
    for b in s.bytes() {
        let valid = b.is_ascii_lowercase()
            || b.is_ascii_uppercase()
            || b.is_ascii_digit()
            || b == b'.'
            || b == b'-'
            || b == b'_';
        if !valid {
            panic!(
                "Invalid character in protocol string (only alphanumeric, '.', '-', '_' are allowed): {}",
                s
            );
        }
    }
}
