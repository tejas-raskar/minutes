# Troubleshooting

This guide helps you diagnose common `minutes` issues in a predictable order.
Start with diagnostics, then move to the section that matches your error.

## Run diagnostics first

Run diagnostics before deeper debugging so you can confirm tool availability and
active PipeWire target resolution.

```bash
minutes doctor
minutes doctor --json
```

If `pipewire_targets` contains `fallback-alias`, target resolution degraded and
capture behavior can vary by host setup.

## BLANK_AUDIO in transcript

This issue usually means no usable audio signal reached the capture pipeline.

Common causes:

- PipeWire or WirePlumber is not available.
- Default sink/source targets did not resolve correctly.
- No system playback occurred during the recording window.

Run these checks:

1. Inspect active audio graph and defaults.

```bash
wpctl status -n
```

2. Verify default sink and source inspection works.

```bash
wpctl inspect @DEFAULT_AUDIO_SINK@
wpctl inspect @DEFAULT_AUDIO_SOURCE@
```

3. Re-run diagnostics.

```bash
minutes doctor
```

## Failed to connect to daemon or connection refused

This error means the CLI cannot reach a running daemon socket.

Use this recovery sequence:

1. Start daemon and verify status.

```bash
minutes daemon start
minutes daemon status
```

2. If state looks stale, restart daemon.

```bash
minutes daemon restart
```

## Gemini returned an error status

This error means the request reached Gemini but the API rejected it.

Common causes:

- Invalid API key.
- Quota or billing limits.
- Invalid model name for your API access.

Run these checks:

1. Confirm a key is present.

```bash
echo "$MINUTES_GEMINI_API_KEY" | wc -c
```

2. Retry summary generation.

```bash
minutes summarize <id>
```

3. Confirm `llm.model` is valid. Current default is `gemini-2.5-flash`.

## Gemini API key is missing

This error means no key was found in config or environment.

Use one of these fixes:

- Set `MINUTES_GEMINI_API_KEY` in your shell environment.
- Set `llm.api_key` in `config.toml`.

Example:

```bash
export MINUTES_GEMINI_API_KEY="your_key_here"
```

## No transcript available yet

This message means recording metadata exists, but transcript segments are not
available yet.

Run these checks:

1. Confirm recording is present.

```bash
minutes list
minutes view <id>
```

2. Confirm Whisper model exists.

```bash
./scripts/install-models.sh base
```

## PipeWire tool missing

If `pw-record` is missing, install PipeWire tools for your distro (often named
`pipewire-tools`).

## Next steps

After fixes, run a fresh smoke flow: `daemon start`, `start`, `stop`,
`summarize`, and `view`.
