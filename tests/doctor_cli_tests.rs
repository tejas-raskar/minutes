use std::process::Command;

fn run_minutes(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_minutes"))
        .args(args)
        .env("RUST_LOG", "error")
        .output()
        .expect("failed to execute minutes")
}

#[test]
fn doctor_subcommand_is_available() {
    let output = run_minutes(&["doctor", "--help"]);

    assert!(
        output.status.success(),
        "doctor --help should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_command_runs() {
    let output = run_minutes(&["doctor"]);

    assert!(
        output.status.success(),
        "doctor should run successfully\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_json_output_is_valid() {
    let output = run_minutes(&["doctor", "--json"]);

    assert!(
        output.status.success(),
        "doctor --json should run successfully\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor --json stdout must be valid JSON");

    assert!(
        value.get("backend").is_some(),
        "expected backend key in doctor json output"
    );
    assert!(
        value.get("checks").is_some(),
        "expected checks key in doctor json output"
    );
}
