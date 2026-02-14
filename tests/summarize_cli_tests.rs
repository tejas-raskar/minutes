use std::process::Command;

#[test]
fn summarize_subcommand_is_available() {
    let output = Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(["summarize", "--help"])
        .output()
        .expect("failed to execute minutes");

    assert!(
        output.status.success(),
        "summarize --help should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn summarize_reports_missing_recording() {
    let output = Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(["summarize", "does-not-exist"])
        .output()
        .expect("failed to execute minutes");

    assert!(
        !output.status.success(),
        "summarize should fail for unknown recording id\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Recording not found"),
        "expected missing recording error, got:\n{}",
        stderr
    );
}
