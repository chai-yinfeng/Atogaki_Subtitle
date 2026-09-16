#!/bin/zsh
set -euo pipefail
setopt NO_BG_NICE

SCRIPT_DIR=${0:A:h}
REPO_ROOT=${SCRIPT_DIR:h}
source "$SCRIPT_DIR/hy-mt2-versions.zsh"

MODEL_SIZE=${1:-1.8b}
EXPERIMENT_ROOT=${ATOGAKI_HY_MT2_ROOT:-$REPO_ROOT/local-artifacts/hy-mt2}
PORT=${ATOGAKI_LLAMA_PORT:-18080}
JOBS=${ATOGAKI_BUILD_JOBS:-$(sysctl -n hw.logicalcpu 2>/dev/null || print 4)}

case "$MODEL_SIZE" in
  1.8b)
    MODEL_REPOSITORY=$HY_MT2_1_8B_REPOSITORY
    MODEL_REVISION=$HY_MT2_1_8B_REVISION
    MODEL_FILE=$HY_MT2_1_8B_FILE
    MODEL_SIZE_BYTES=$HY_MT2_1_8B_SIZE
    MODEL_SHA256=$HY_MT2_1_8B_SHA256
    ;;
  7b)
    MODEL_REPOSITORY=$HY_MT2_7B_REPOSITORY
    MODEL_REVISION=$HY_MT2_7B_REVISION
    MODEL_FILE=$HY_MT2_7B_FILE
    MODEL_SIZE_BYTES=$HY_MT2_7B_SIZE
    MODEL_SHA256=$HY_MT2_7B_SHA256
    ;;
  *)
    print -u2 "usage: $0 [1.8b|7b]"
    exit 2
    ;;
esac

for command_name in cmake curl jq shasum tar; do
  if ! command -v "$command_name" >/dev/null; then
    print -u2 "missing validation dependency: $command_name"
    exit 1
  fi
done

CACHE_DIR="$EXPERIMENT_ROOT/cache"
SOURCE_DIR="$EXPERIMENT_ROOT/llama.cpp-$LLAMA_CPP_COMMIT"
BUILD_DIR="$SOURCE_DIR/build-atogaki"
MODEL_DIR="$EXPERIMENT_ROOT/models"
RESULT_DIR="$EXPERIMENT_ROOT/results"
ARCHIVE="$CACHE_DIR/llama.cpp-$LLAMA_CPP_COMMIT.tar.gz"
MODEL_PATH="$MODEL_DIR/$MODEL_FILE"
MODEL_URL="https://huggingface.co/$MODEL_REPOSITORY/resolve/$MODEL_REVISION/$MODEL_FILE"
SERVER="$BUILD_DIR/bin/llama-server"
LOG_PATH="$RESULT_DIR/$MODEL_SIZE-server.log"
RESPONSE_PATH="$RESULT_DIR/$MODEL_SIZE-structure-response.json"

mkdir -p "$CACHE_DIR" "$MODEL_DIR" "$RESULT_DIR"

verify_sha256() {
  local file_path=$1
  local expected=$2
  local actual
  actual=$(shasum -a 256 "$file_path" | awk '{print $1}')
  if [[ "$actual" != "$expected" ]]; then
    print -u2 "SHA-256 mismatch for $file_path"
    print -u2 "expected: $expected"
    print -u2 "actual:   $actual"
    return 1
  fi
}

if [[ ! -f "$ARCHIVE" ]] || ! verify_sha256 "$ARCHIVE" "$LLAMA_CPP_SOURCE_SHA256"; then
  rm -f "$ARCHIVE"
  curl -fL --retry 3 "$LLAMA_CPP_SOURCE_URL" -o "$ARCHIVE"
  verify_sha256 "$ARCHIVE" "$LLAMA_CPP_SOURCE_SHA256"
fi

if [[ ! -d "$SOURCE_DIR" ]]; then
  tar -xzf "$ARCHIVE" -C "$EXPERIMENT_ROOT"
fi

cmake -S "$SOURCE_DIR" -B "$BUILD_DIR" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF \
  -DGGML_NATIVE=OFF \
  -DLLAMA_BUILD_COMMIT="$LLAMA_CPP_COMMIT" \
  -DLLAMA_BUILD_NUMBER=0 \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_SERVER=ON \
  -DLLAMA_BUILD_UI=OFF \
  -DLLAMA_USE_PREBUILT_UI=OFF \
  -DLLAMA_OPENSSL=OFF
cmake --build "$BUILD_DIR" --config Release --target llama-server -j "$JOBS"

if ! "$SERVER" --version 2>&1 | grep -F "$LLAMA_CPP_COMMIT" >/dev/null; then
  print -u2 "llama-server does not report the pinned source commit"
  exit 1
fi
if command -v otool >/dev/null && otool -L "$SERVER" | grep -E '/opt/homebrew|/usr/local' >/dev/null; then
  print -u2 "llama-server has a build-machine dependency"
  otool -L "$SERVER" >&2
  exit 1
fi

if [[ ! -f "$MODEL_PATH" ]] || ! verify_sha256 "$MODEL_PATH" "$MODEL_SHA256"; then
  rm -f "$MODEL_PATH"
  curl -fL --retry 3 --continue-at - "$MODEL_URL" -o "$MODEL_PATH"
  verify_sha256 "$MODEL_PATH" "$MODEL_SHA256"
fi

ACTUAL_SIZE=$(stat -f %z "$MODEL_PATH" 2>/dev/null || stat -c %s "$MODEL_PATH")
if [[ "$ACTUAL_SIZE" != "$MODEL_SIZE_BYTES" ]]; then
  print -u2 "unexpected model size: $ACTUAL_SIZE (expected $MODEL_SIZE_BYTES)"
  exit 1
fi

"$SERVER" \
  --host 127.0.0.1 \
  --port "$PORT" \
  --model "$MODEL_PATH" \
  --ctx-size 8192 \
  --jinja \
  --n-gpu-layers 99 \
  --api-key atogaki-local-validation \
  >"$LOG_PATH" 2>&1 &
SERVER_PID=$!
cleanup() {
  if kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID"
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

for _ in {1..180}; do
  if curl -fsS "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
    break
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    print -u2 "llama-server exited before becoming healthy; see $LOG_PATH"
    exit 1
  fi
  sleep 1
done
curl -fsS "http://127.0.0.1:$PORT/health" >/dev/null

REQUEST=$(jq -n --arg model "$MODEL_FILE" '{
  model: $model,
  messages: [
    {
      role: "system",
      content: "Translate Japanese spoken-language subtitle cues into Simplified Chinese. Return JSON only. Preserve each segment_id exactly once and preserve [[ATOGAKI_TERM_0]] exactly. Schema: {\"translations\":[{\"segment_id\":\"...\",\"translated_text\":\"...\"}]}"
    },
    {
      role: "user",
      content: "Context: 京都の吹奏楽部について話しています。\nTarget cues: [{\"segment_id\":\"cue-1\",\"source_text\":\"[[ATOGAKI_TERM_0]]に入ってから、\"},{\"segment_id\":\"cue-2\",\"source_text\":\"毎日が本当に楽しいです。\"}]"
    }
  ],
  temperature: 0.7,
  top_p: 0.6,
  top_k: 20,
  repeat_penalty: 1.05,
  max_tokens: 512,
  response_format: {type: "json_object"},
  stream: false
}')

curl -fsS \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer atogaki-local-validation' \
  --data "$REQUEST" \
  "http://127.0.0.1:$PORT/v1/chat/completions" \
  > "$RESPONSE_PATH"

CONTENT=$(jq -er '.choices[0].message.content' "$RESPONSE_PATH")
print -r -- "$CONTENT" | jq -e '
  .translations as $items |
  ($items | length == 2) and
  ([$items[].segment_id] | sort == ["cue-1", "cue-2"]) and
  ([$items[].translated_text] | all(length > 0)) and
  ([$items[].translated_text] | join("") | contains("[[ATOGAKI_TERM_0]]"))
' >/dev/null

print "Hy-MT2 $MODEL_SIZE validation passed"
print "  runtime: $LLAMA_CPP_VERSION ($LLAMA_CPP_COMMIT)"
print "  model:   $MODEL_REPOSITORY@$MODEL_REVISION/$MODEL_FILE"
print "  response: $RESPONSE_PATH"
