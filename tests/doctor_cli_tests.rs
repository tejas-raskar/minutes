use std::process::Command;

#[test]
fn doctor_subcommand_is_available() {
    let output = Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(["doctor", "--help"])
        .output()
        .expect("failed to execute minutes");

    assert!(
        output.status.success(),
        "doctor --help should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_command_runs() {
    let output = Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(["doctor"])
        .output()
        .expect("failed to execute minutes");

    assert!(
        output.status.success(),
        "doctor should run successfully\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
