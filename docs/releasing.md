# macOS 发布说明

_最后更新：2026-08-06_

## 产物边界

DMG 不提交到 Git 历史，也不使用 Git LFS。源码提交并打 tag 后，把 DMG 作为 GitHub Release asset 上传。这样仓库保持轻量，Release 页面仍能为测试者提供固定版本下载。

首个 Apple Silicon 预发布建议包含：

- `Atogaki-v0.1.0-alpha.1-macos-arm64.dmg`
- `Atogaki-v0.1.0-alpha.1-macos-arm64.dmg.sha256`
- `Atogaki-0.1.0-third-party-sources.tar.xz`
- `Atogaki-0.1.0-third-party-sources.tar.xz.sha256`

`src-tauri/third-party/` 中的项目依赖声明、sidecar 构建清单和许可证材料已经随 `.app` 进入 DMG；对应源码包单独上传，避免每个普通用户重复下载约数十 MiB 的源码。

GitHub 自动生成的 Source code 归档只覆盖本仓库，不能替代 FFmpeg、whisper.cpp 与静态依赖的对应源码材料。

## 首次手工预发布

1. 在干净提交上完成 Rust、前端、打包 App、模型下载和真实窗口回归。
2. 运行 `./scripts/generate-rust-licenses.sh` 与 `node ./scripts/generate-frontend-licenses.mjs`，审阅并提交生成声明。详细范围见 `docs/third-party-license-audit.md`。
3. 用固定 sidecar 构建 DMG。`CI=true tauri build --bundles dmg` 只用于结构 smoke，会跳过 Finder 图标布局，不能作为最终发布产物；最终 DMG 必须在非 CI 环境运行 `tauri build --bundles dmg`，挂载后确认 App 位于左侧、Applications 位于右侧，两个图标在 660×400 窗口中居中并充分分隔。若 Finder 美化脚本挂起，应阻止发布并单独排查，不能回退发布左上角堆叠的 CI 产物。当前配置使用不需要 Apple Developer 账号的 ad-hoc identity `-`，发布时必须明确标注“ad-hoc 签名、未公证”，供知情测试者使用。
4. 运行 `./scripts/package-sidecar-sources-macos.sh`。进入输出目录执行 `shasum -a 256 -c Atogaki-0.1.0-third-party-sources.tar.xz.sha256`，并抽查归档的 `SOURCES.md`、`sources/SHA256SUMS` 和 `build/build-manifest.txt`。
5. 为最终 DMG 生成 SHA-256，并挂载确认 App、Applications 链接、三个 sidecar、根许可证和 `third-party/` 声明都存在。
6. 创建带版本号的 annotated tag，例如 `v0.1.0-alpha.1`，并把 tag 推送到 GitHub。
7. 在 GitHub 的 Releases 页面从该 tag 创建 prerelease，填写支持架构、macOS 版本、已知 Gatekeeper 操作、校验值、模型不内置和第三方许可证说明。
8. 上传 DMG、两个 SHA-256 文件与对应源码包，而不是把这些大产物 `git add` 到仓库。

可使用 GitHub CLI 上传已核对的产物：

```bash
gh release create v0.1.0-alpha.1 \
  path/to/Atogaki-v0.1.0-alpha.1-macos-arm64.dmg \
  path/to/Atogaki-v0.1.0-alpha.1-macos-arm64.dmg.sha256 \
  src-tauri/target/release/bundle/sources/Atogaki-0.1.0-third-party-sources.tar.xz \
  src-tauri/target/release/bundle/sources/Atogaki-0.1.0-third-party-sources.tar.xz.sha256 \
  --prerelease --title "Atogaki v0.1.0-alpha.1" --notes-file path/to/release-notes.md
```

## 自动化时机

首轮建议手工发布以稳定构建清单和窗口回归。取得 Apple Developer Program 资格后，再让 GitHub Actions 在版本 tag 上构建、签名、公证、装订 notarization ticket、生成 DMG 和校验文件，并上传到同一个 Release。签名证书、App Store Connect API key 等只放 GitHub Actions encrypted secrets，不进入仓库。

当前 Apple Silicon App 使用 ad-hoc 签名并声明最低 macOS 12.0；这可以保证 Bundle 完整性，但不能代替 Developer ID 签名与公证，也不会消除外部下载时的 Gatekeeper 提示。DMG 已在本机用 `hdiutil verify` 通过结构校验，并确认包含 `.app`、Applications 链接、三个 sidecar、Apache-2.0 项目许可证和第三方构建清单。Tauri 配置现已固定 660×400 Finder 窗口及“App 左、Applications 右”的图标位置；`CI=true` 的无美化产物仍只用于结构校验。当前 macOS 26 环境的非 CI Finder 美化脚本仍需在最终发布前实机构建并复核，自动化不能用无布局产物替代这项发布门禁。
