# Configuration

This guide explains how `minutes` loads configuration, how to initialize a
config file, and what each key controls.

## Config sources

`minutes` reads configuration from file first, then applies environment overrides
for selected keys.

- Config file path: `~/.config/minutes/config.toml` (XDG path).
- Environment override: `MINUTES_GEMINI_API_KEY`.
- If no config file exists, built-in defaults are used.

## Initialize config

Run these steps to create and inspect your local config file.

1. Print the active config path.

```bash
minutes config path
```

2. Initialize a default config file.

```bash
minutes config init
```

3. Print the loaded config values.

```bash
minutes config show
```

## Example config.toml

Use this example as a baseline and then adjust values for your environment.

```toml
[general]
data_dir = "/home/you/.local/share/minutes"
log_level = "info"

[audio]
backend = "auto"                 # auto | pipewire | cpal
sample_rate = 16000
channels = 1
capture_system = true
capture_microphone = true
device = ""
compress_to_ogg = true
ogg_bitrate = 24000
mic_boost = 1.2

[whisper]
model = "base"                   # tiny | base | small | medium | large
models_dir = "/home/you/.local/share/minutes/models"
language = ""
translate = false
threads = 0

[llm]
provider = "gemini"
api_key = ""
model = "gemini-2.5-flash"
endpoint = ""

[tui]
show_timestamps = true
recent_count = 5
theme = "dark"
```

## Key behavior notes

These notes explain the most important runtime behaviors for common setups.

- `audio.backend = "auto"` selects PipeWire when available.
- `audio.backend = "cpal"` is microphone-focused and is not the preferred path
  for full system + mic meeting capture.
- `llm.provider` currently supports `gemini`.
- If `llm.api_key` is empty in config, `MINUTES_GEMINI_API_KEY` is used when
  available.

## Next steps

After configuration is in place, run `minutes doctor` and then execute a full
recording flow with `start`, `stop`, and `summarize`.
