use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
fn command_exists(bin: &str) -> bool {
    Command::new(bin)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[cfg(target_os = "linux")]
fn parse_wpctl_node_id(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed = line.trim();
        let id = trimmed.strip_prefix("id ")?.split(',').next()?.trim();
        if id.chars().all(|c| c.is_ascii_digit()) {
            Some(id.to_string())
        } else {
            None
        }
    })
}

#[cfg(target_os = "linux")]
fn resolve_default_sink_id() -> Option<String> {
    let output = Command::new("wpctl")
        .args(["inspect", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_wpctl_node_id(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "linux")]
fn generate_tone(path: &Path) -> std::io::Result<()> {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=2",
            "-ac",
            "1",
            "-ar",
            "16000",
        ])
        .arg(path)
        .status()?;
    assert!(status.success(), "failed to generate test tone");
    Ok(())
}

#[cfg(target_os = "linux")]
fn restore_mute_from_volume_output(before: &str) {
    let mute_state = if before.contains("MUTED") { "1" } else { "0" };
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", mute_state])
        .status();
}

#[cfg(target_os = "linux")]
fn capture_rms_db(path: &Path) -> Option<f32> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-i",
            path.to_str()?,
            "-af",
            "astats=metadata=1:reset=0",
            "-f",
            "null",
            "-",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stderr);

    for line in text.lines() {
        let marker = "RMS level dB:";
        if let Some(value) = line.split(marker).nth(1) {
            let trimmed = value.trim();
            if trimmed != "-inf" {
                if let Ok(parsed) = trimmed.parse::<f32>() {
                    return Some(parsed);
                }
            }
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn terminate_record(mut child: Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let _ = child.wait();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires host PipeWire and real audio devices; run manually"]
fn muted_mic_still_captures_system_audio() {
    if !command_exists("wpctl")
        || !command_exists("pw-record")
        || !command_exists("pw-play")
        || !command_exists("ffmpeg")
    {
        eprintln!("skipping: required audio tools are unavailable");
        return;
    }

    let sink_id = match resolve_default_sink_id() {
        Some(id) => id,
        None => {
            eprintln!("skipping: could not resolve default sink id");
            return;
        }
    };

    let workdir = tempfile::tempdir().expect("failed to create tempdir");
    let tone_path: PathBuf = workdir.path().join("tone.wav");
    let capture_path: PathBuf = workdir.path().join("capture.wav");

    generate_tone(&tone_path).expect("failed generating tone");

    let mute_before = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
        .output()
        .expect("failed to read source mute state");
    let mute_before_stdout = String::from_utf8_lossy(&mute_before.stdout).to_string();

    let mute_status = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "1"])
        .status()
        .expect("failed to mute microphone source");
    assert!(mute_status.success(), "failed to mute microphone source");

    let record_child = Command::new("pw-record")
        .args([
            "--target",
            &sink_id,
            "--rate",
            "16000",
            "--channels",
            "1",
            "--format",
            "s16",
            capture_path
                .to_str()
                .expect("capture path should be valid utf-8"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start pw-record");

    thread::sleep(Duration::from_millis(800));

    let play_status = Command::new("pw-play")
        .arg(tone_path.to_str().expect("tone path should be valid utf-8"))
        .status()
        .expect("failed to play tone");
    assert!(play_status.success(), "failed to play tone");

    thread::sleep(Duration::from_millis(400));
    terminate_record(record_child);
    restore_mute_from_volume_output(&mute_before_stdout);

    let rms_db = capture_rms_db(&capture_path).expect("expected measurable non-silent audio");
    assert!(
        rms_db > -80.0,
        "captured audio appears too quiet / silent (RMS {} dB)",
        rms_db
    );
}
