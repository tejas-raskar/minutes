#!/bin/bash
# Download Whisper models for minutes

set -e

# Default model and directory
MODEL="${1:-base}"
MODELS_DIR="${MINTUES_MODELS_DIR:-$HOME/.local/share/minutes/models}"

# Available models
MODELS=("tiny" "tiny.en" "base" "base.en" "small" "small.en" "medium" "medium.en" "large-v1" "large-v2" "large-v3")

# Check if model is valid
valid_model=false
for m in "${MODELS[@]}"; do
    if [[ "$MODEL" == "$m" ]]; then
        valid_model=true
        break
    fi
done

if [[ "$valid_model" == false ]]; then
    echo "Invalid model: $MODEL"
    echo "Available models: ${MODELS[*]}"
    exit 1
fi

# Create models directory
mkdir -p "$MODELS_DIR"

# Download URL
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
FILENAME="ggml-${MODEL}.bin"
URL="${BASE_URL}/${FILENAME}"
OUTPUT="${MODELS_DIR}/${FILENAME}"

# Check if already exists
if [[ -f "$OUTPUT" ]]; then
    echo "Model already exists: $OUTPUT"
    echo "Remove it first if you want to re-download."
    exit 0
fi

echo "Downloading Whisper model: $MODEL"
echo "URL: $URL"
echo "Output: $OUTPUT"
echo

# Download with progress
if command -v curl &> /dev/null; then
    curl -L --progress-bar "$URL" -o "$OUTPUT"
elif command -v wget &> /dev/null; then
    wget --show-progress "$URL" -O "$OUTPUT"
else
    echo "Error: curl or wget is required"
    exit 1
fi

echo
echo "Model downloaded successfully: $OUTPUT"
echo
echo "Model sizes for reference:"
echo "  tiny    - ~75 MB  (fastest, least accurate)"
echo "  base    - ~142 MB (good balance)"
echo "  small   - ~466 MB"
echo "  medium  - ~1.5 GB"
echo "  large   - ~2.9 GB (slowest, most accurate)"
