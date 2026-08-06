#!/bin/zsh
set -euo pipefail

SCRIPT_DIR=${0:A:h}
PROJECT_DIR=${SCRIPT_DIR:h}
CARGO_ABOUT=${CARGO_ABOUT:-cargo-about}
OUTPUT=${1:-$PROJECT_DIR/src-tauri/third-party/rust-licenses.html}

if ! command -v "$CARGO_ABOUT" >/dev/null 2>&1; then
  print -u2 "cargo-about 0.9.1 is required; install it with:"
  print -u2 "  cargo install --locked --features cli --version 0.9.1 cargo-about"
  exit 1
fi
if ! command -v perl >/dev/null 2>&1; then
  print -u2 "Perl is required to normalize generated license text"
  exit 1
fi

if [[ "$($CARGO_ABOUT --version)" != "cargo-about 0.9.1" ]]; then
  print -u2 "cargo-about 0.9.1 is required for reproducible license output"
  exit 1
fi

mkdir -p "${OUTPUT:h}"
"$CARGO_ABOUT" generate "$PROJECT_DIR/about.hbs" \
  --config "$PROJECT_DIR/about.toml" \
  --manifest-path "$PROJECT_DIR/src-tauri/Cargo.toml" \
  --target aarch64-apple-darwin \
  --locked --offline --fail \
  --output-file "$OUTPUT"

# Upstream license files sometimes contain CRLF or trailing spaces. Normalize
# only line endings and trailing horizontal whitespace so git diff --check can
# remain a release gate without changing the license wording.
perl -pi -e 's/\r$//; s/[ \t]+$//' "$OUTPUT"

print "Generated Rust license notices: $OUTPUT"
