# Changelog

This file records notable project changes by released version.

## [0.1.0] - 2026-02-16

This is the initial public, CLI-first release.

### Added

This release introduces the first complete end-to-end CLI workflow.

- CLI recording lifecycle: `start`, `stop`, `status`, `list`, `view`, `search`,
  and `export`.
- Daemon management: `daemon start|stop|restart|status`.
- Local Whisper transcription pipeline.
- Audio backend abstraction across PipeWire and cpal.
- OGG compression pipeline.
- Gemini-based summary command: `summarize`.
- `doctor` diagnostics command with optional `--json` output.
- PipeWire reliability integration test for muted-mic/system-audio capture
  (ignored by default).

### Changed

This release also improves default model selection and audio target resolution.

- PipeWire target resolution now uses `wpctl inspect` first.
- PipeWire fallback resolution now parses `wpctl status -n`.
- PipeWire fallback now matches configured defaults when `*` markers are not
  present.
- Default Gemini model is now `gemini-2.5-flash`.

### Fixed

Several reliability issues were corrected in this release.

- Daemon start now fails cleanly when background boot fails.
- System track is preserved when microphone capture is unavailable.
- System-audio capture reliability is improved when the microphone is muted.

## [Unreleased]

This section tracks changes that are not released yet.

- No unreleased entries yet.
