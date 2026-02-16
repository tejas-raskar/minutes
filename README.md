# minutes

`minutes` is a Linux-first CLI for meeting recording, transcription, and AI
summaries. This guide gives you a practical setup path, core command flow, and
links to deeper documentation for configuration, troubleshooting, and release
operations.

## What minutes does

`minutes` focuses on a small, reliable workflow for capturing meetings and
turning them into searchable notes.

- Record system audio and microphone audio.
- Transcribe recordings locally with Whisper.
- Store transcripts and metadata in a local SQLite database.
- Generate and persist one summary per recording with Gemini.

## Current scope

The production path is CLI-first. A TUI is available, but the core release
workflow is the CLI command set.

- Primary UX is the CLI.
- TUI exists as an optional interface.
- LLM provider abstraction is in place; Gemini is implemented.

## Requirements

Before you run `minutes`, make sure your environment has the required runtime
and tools.

- Linux.
- Rust toolchain with `cargo`.
- PipeWire runtime.
- `pw-record` and `pw-play` (`pipewire-tools` on many distros).
- A Whisper model file, for example `ggml-base.bin`.

## Install options

You can either download a prebuilt binary from GitHub Releases or build from
source.

### Option 1: Use a prebuilt Linux binary

Each release publishes Linux tarballs for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`

Download the matching archive from the project's Releases page, extract it, and
put `bin/minutes` on your `PATH`.

### Option 2: Build from source

```bash
cd minutes
cargo build --bin minutes
```

## Quick start

Use these steps to run a complete local setup and first recording.

If you built from source, run commands as `cargo run -- <command>`. If you
installed a release binary, run them as `minutes <command>`.

1. Install a Whisper model.

```bash
./scripts/install-models.sh base
```

2. Set your Gemini API key for summaries.

```bash
export MINUTES_GEMINI_API_KEY="your_key_here"
```

3. Run diagnostics to verify your audio environment.

```bash
cargo run -- doctor
cargo run -- doctor --json
```

4. Start the daemon and record a meeting.

```bash
cargo run -- daemon start
cargo run -- start -t "Team sync"
# speak and/or play meeting audio
cargo run -- stop
```

5. List, inspect, and summarize the recording.

```bash
cargo run -- list
cargo run -- view <recording_id_prefix>
cargo run -- summarize <recording_id_prefix>
```

## Command reference

This list summarizes the main command surface in `0.1.0`.

- `minutes start`
- `minutes stop`
- `minutes status`
- `minutes list`
- `minutes view <id>`
- `minutes search <query>`
- `minutes summarize <id>`
- `minutes doctor [--json]`
- `minutes export <id> --format txt|json|srt`
- `minutes daemon start|stop|restart|status`
- `minutes config show|path|init`

## Configuration

By default, `minutes` runs with built-in values when no config file exists.
Initialize config if you want to customize behavior explicitly.

```bash
cargo run -- config path
cargo run -- config init
cargo run -- config show
```

For full examples and field descriptions, see `docs/CONFIG.md`.

## Summary provider

Summary generation uses the configured LLM provider.

- Default provider is `gemini`.
- Default model is `gemini-2.5-flash`.
- Summary generation fails if no Gemini API key is configured.

## Troubleshooting

If recording, daemon, or summary behavior is unexpected, start with diagnostics
and then follow targeted fixes.

See `docs/TROUBLESHOOTING.md`.

## Release notes

For released changes, see `CHANGELOG.md`.

## Next steps

Download the latest published binaries from GitHub Releases, then follow the
quick start flow above to validate audio capture and summarization on your
machine.
