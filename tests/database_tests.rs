use anyhow::Result;
use tempfile::tempdir;

use minutes::storage::{Database, Recording, RecordingState, TranscriptSegment};

#[test]
fn database_supports_core_recording_workflow() -> Result<()> {
    let tmp = tempdir()?;
    let db_path = tmp.path().join("minutes.db");
    let db = Database::open_path(&db_path)?;

    let recording = Recording::new("Team sync".to_string());
    db.insert_recording(&recording)?;

    db.update_recording_state(&recording.id, RecordingState::Pending)?;
    let pending = db.get_pending_recordings()?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, recording.id);

    db.update_recording_state(&recording.id, RecordingState::Transcribing)?;

    let segment1 = TranscriptSegment::new(
        recording.id.clone(),
        0.0,
        5.0,
        "Hello team this is a test meeting".to_string(),
    );
    let segment2 =
        TranscriptSegment::new(recording.id.clone(), 5.0, 8.0, "Agenda and follow up".to_string());
    db.insert_segments(&[segment1, segment2])?;

    let segments = db.get_transcript_segments(&recording.id)?;
    assert_eq!(segments.len(), 2);

    let results = db.search_transcripts("hello", 10)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.id, recording.id);
    assert!(results[0].1.text.to_lowercase().contains("hello"));

    db.update_recording_state(&recording.id, RecordingState::Completed)?;
    let final_recording = db
        .get_recording(&recording.id)?
        .expect("recording should still exist");
    assert_eq!(final_recording.state, RecordingState::Completed);

    Ok(())
}

#[test]
fn deleting_recording_removes_transcript_segments() -> Result<()> {
    let tmp = tempdir()?;
    let db_path = tmp.path().join("minutes.db");
    let db = Database::open_path(&db_path)?;

    let recording = Recording::new("Delete me".to_string());
    db.insert_recording(&recording)?;

    let segment =
        TranscriptSegment::new(recording.id.clone(), 0.0, 2.0, "Temporary content".to_string());
    db.insert_segment(&segment)?;

    db.delete_recording(&recording.id)?;

    let remaining = db.get_transcript_segments(&recording.id)?;
    assert!(remaining.is_empty());
    assert!(db.get_recording(&recording.id)?.is_none());

    Ok(())
}
