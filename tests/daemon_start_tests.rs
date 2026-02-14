use std::process::Command;

#[test]
fn daemon_start_fails_when_background_daemon_fails_to_boot() {
    let output = Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(["daemon", "start"])
        .env("XDG_RUNTIME_DIR", "/dev/null")
        .output()
        .expect("failed to execute minutes");

    assert!(
        !output.status.success(),
        "daemon start unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
