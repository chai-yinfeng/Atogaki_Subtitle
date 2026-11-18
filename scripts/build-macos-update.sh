#!/bin/zsh
set -euo pipefail

PROJECT_DIR=${0:A:h:h}
RELEASE_OUT=${1:?用法：scripts/build-macos-update.sh /绝对路径/发行目录}
[[ "$RELEASE_OUT" = /* ]] || { print -u2 '请输入绝对输出路径。'; exit 1; }
[[ "$(uname -s)-$(uname -m)" = Darwin-arm64 ]] || { print -u2 '需要 Apple Silicon macOS。'; exit 1; }
tauri_cli() {
  if [[ -n ${ATOGAKI_TAURI_CLI:-} ]]; then
    "$ATOGAKI_TAURI_CLI" "$@"
  else
    cargo tauri "$@"
  fi
}
if [[ "$(tauri_cli signer sign --help)" != *--app-version* ]]; then
  print -u2 '需要支持 --app-version 的 Tauri CLI（2.12 或更新）。'
  exit 1
fi
: "${ATOGAKI_UPDATER_PUBLIC_KEY:?请提供发行公钥的内容}"
: "${TAURI_SIGNING_PRIVATE_KEY:?请从发行密钥存储提供私钥}"
cd "$PROJECT_DIR/src-tauri"
VERSION=$(node -p 'require("./tauri.conf.json").version')
[[ "$VERSION" =~ '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' ]] || { print -u2 '更新只接受正式三段版本。'; exit 1; }
SIDECARS=(ffmpeg ffprobe whisper-cli llama-server atogaki-audio-tap)
for SIDECAR in "${SIDECARS[@]}"; do
  INPUT="binaries/$SIDECAR-aarch64-apple-darwin"
  [[ -x "$INPUT" && ! -L "$INPUT" ]] || { print -u2 "请准备独立的发行输入：$INPUT"; exit 1; }
done

OVERLAY=$(mktemp "${TMPDIR%/}/atogaki-update.XXXXXXXX")
trap 'rm -f "$OVERLAY"' EXIT
node -e 'console.log(JSON.stringify({bundle:{createUpdaterArtifacts:true},plugins:{updater:{pubkey:process.env.ATOGAKI_UPDATER_PUBLIC_KEY.trim(),requireSignedVersion:true}}}))' > "$OVERLAY"
# A JSON string is accepted by --config and avoids extension-dependent parsing.
tauri_cli build --config tauri.macos-complete.conf.json --config "$(cat "$OVERLAY")" --bundles app,dmg -- --locked
TARGET_DIR=$(cargo metadata --no-deps --format-version 1 --locked | node -pe 'JSON.parse(require("fs").readFileSync(0,"utf8")).target_directory')
BUNDLE="$TARGET_DIR/release/bundle"
APP="$BUNDLE/macos/Atogaki.app"
for SIDECAR in "${SIDECARS[@]}"; do
  [[ -x "$APP/Contents/MacOS/$SIDECAR" ]] || { print -u2 "缺少完整安装包组件：$SIDECAR"; exit 1; }
done
codesign --verify --deep --strict "$APP"
ASSET="Atogaki-v$VERSION-macos-arm64.app.tar.gz"
MANIFEST=$(node --input-type=module - "$VERSION" "$ASSET" "$APP.tar.gz.sig" <<'JS'
import {readFileSync} from "node:fs";
const [version, asset, sigFile] = process.argv.slice(2);
const signature = readFileSync(sigFile, "utf8").trim();
const trustedComment = Buffer.from(signature, "base64").toString("utf8").split("\n").find(line => line.startsWith("trusted comment: "));
if (!trustedComment?.split("\t").includes(`version:${version}`)) throw Error("签名没有绑定当前发行版本");
const url = `https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/download/v${version}/${asset}`;
console.log(JSON.stringify({version, platforms:{"darwin-aarch64":{signature, url}}}, null, 2));
JS
)
mkdir -p "$RELEASE_OUT"
cp "$APP.tar.gz" "$RELEASE_OUT/$ASSET"
cp "$APP.tar.gz.sig" "$RELEASE_OUT/$ASSET.sig"
cp "$BUNDLE/dmg/Atogaki_${VERSION}_aarch64.dmg" "$RELEASE_OUT/Atogaki-v$VERSION-macos-arm64.dmg"
print -r -- "$MANIFEST" > "$RELEASE_OUT/latest.json"
cd "$RELEASE_OUT"
shasum -a 256 "Atogaki-v$VERSION-macos-arm64.dmg" > "Atogaki-v$VERSION-macos-arm64.dmg.sha256"
print "已生成 v$VERSION 发行包和更新清单：$RELEASE_OUT"
