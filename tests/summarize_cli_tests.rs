mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;

use common::{run_minutes, TestEnv};
use minutes::config::Settings;
use minutes::storage::{Database, Recording, TranscriptSegment};
use tempfile::TempDir;

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

fn seed_recording(data_dir: &Path) -> String {
    let mut settings = Settings::default();
    settings.general.data_dir = data_dir.to_path_buf();

    let db = Database::open(&settings).expect("open test database");
    let recording = Recording::new("Summary regression".to_string());
    db.insert_recording(&recording)
        .expect("insert test recording");
    db.insert_segment(&TranscriptSegment::new(
        recording.id.clone(),
        0.0,
        4.0,
        "Discussed rollout and launch timeline.".to_string(),
    ))
    .expect("insert test segment");

    recording.id
}

fn setup_summary_env(
    env: &TestEnv,
    data_dir: &Path,
    endpoint: &str,
    api_key: &str,
) -> String {
    let recording_id = seed_recording(data_dir);
    let config = format!(
        r#"[general]
data_dir = "{data_dir}"
log_level = "error"

[llm]
provider = "gemini"
api_key = "{api_key}"
model = "gemini-2.5-flash"
endpoint = "{endpoint}"
"#,
        data_dir = toml_path(data_dir),
        api_key = api_key,
        endpoint = endpoint,
    );
    env.write_config(&config);
    recording_id
}

fn spawn_fake_gemini_server(status: &str, body: &str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake server");
    let address = listener.local_addr().expect("read fake server address");
    let status = status.to_string();
    let body = body.to_string();

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept fake client");
        let mut request_buf = [0_u8; 8192];
        let _ = stream.read(&mut request_buf);

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
            status = status,
            len = body.len(),
            body = body,
        );
        stream
            .write_all(response.as_bytes())
            .expect("write fake response");
    });

    (format!("http://{}/v1beta", address), handle)
}

#[test]
fn summarize_subcommand_is_available() {
    let output = run_minutes(&["summarize", "--help"]);

    assert!(
        output.status.success(),
        "summarize --help should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn summarize_reports_missing_recording() {
    let output = run_minutes(&["summarize", "does-not-exist"]);

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

#[test]
fn summarize_success_is_persisted_and_visible_in_view() {
    let env = TestEnv::new();
    let data_dir = TempDir::new().expect("create data dir");
    let (endpoint, server) = spawn_fake_gemini_server(
        "200 OK",
        r###"{"candidates":[{"content":{"parts":[{"text":"## Summary\n- Launch moved to Friday."}]}}]}"###,
    );

    let recording_id = setup_summary_env(&env, data_dir.path(), &endpoint, "test-key");
    let output = env.run(&["summarize", &recording_id[..8]]);
    server.join().expect("join fake server");

    assert!(
        output.status.success(),
        "summarize should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summarize_stdout = String::from_utf8_lossy(&output.stdout);
    assert!(summarize_stdout.contains("Summary saved for"));
    assert!(summarize_stdout.contains("View it with: minutes view"));
    assert!(summarize_stdout.contains("Launch moved to Friday."));

    let view = env.run(&["view", &recording_id[..8]]);
    assert!(
        view.status.success(),
        "view should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&view.stdout),
        String::from_utf8_lossy(&view.stderr)
    );

    let view_stdout = String::from_utf8_lossy(&view.stdout);
    assert!(view_stdout.contains("Summary:"));
    assert!(view_stdout.contains("Transcript:"));
    assert!(view_stdout.contains("Launch moved to Friday."));
}

#[test]
fn summarize_reports_gemini_error_body_details() {
    let env = TestEnv::new();
    let data_dir = TempDir::new().expect("create data dir");
    let (endpoint, server) = spawn_fake_gemini_server(
        "404 Not Found",
        r#"{"error":{"code":404,"message":"models/gemini-2.5-flash is not found for API version v1beta","status":"NOT_FOUND"}}"#,
    );

    let recording_id = setup_summary_env(&env, data_dir.path(), &endpoint, "test-key");
    let output = env.run(&["summarize", &recording_id[..8]]);
    server.join().expect("join fake server");

    assert!(
        !output.status.success(),
        "summarize should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("models/gemini-2.5-flash is not found"),
        "stderr should include Gemini API error body details, got:\n{}",
        stderr
    );
}
