# 发布说明

_最后更新：2026-08-23_

macOS Apple Silicon 是日常开发与完整体验的质量基线。Windows 11 x86_64 已建立未签名预发布基线，按选定稳定候选集中构建和实机回归；两个平台共享产品代码，但不要求每次 macOS 开发提交都同步生成 Windows 安装包。

## macOS Apple Silicon

### 产物边界

DMG 不提交到 Git 历史，也不使用 Git LFS。源码提交并打 tag 后，把 DMG 作为 GitHub Release asset 上传。这样仓库保持轻量，Release 页面仍能为测试者提供固定版本下载。

首个 Apple Silicon 预发布建议包含：

- `Atogaki-v0.1.0-alpha.1-macos-arm64.dmg`
- `Atogaki-v0.1.0-alpha.1-macos-arm64.dmg.sha256`
- `Atogaki-0.1.0-third-party-sources.tar.xz`
- `Atogaki-0.1.0-third-party-sources.tar.xz.sha256`

`src-tauri/third-party/` 中的项目依赖声明、sidecar 构建清单和许可证材料已经随 `.app` 进入 DMG；对应源码包单独上传，避免每个普通用户重复下载约数十 MiB 的源码。

GitHub 自动生成的 Source code 归档只覆盖本仓库，不能替代 FFmpeg、whisper.cpp 与静态依赖的对应源码材料。

### 首次手工预发布

1. 在干净提交上完成 Rust、前端、打包 App、模型下载和真实窗口回归。凡本版本新增或修改的交互，必须在**最终 ad-hoc 签名 App**中逐项完成对应 `docs/desktop-testing.md` 场景并记录结果；仅通过单元测试、前端构建、Tauri 编译，或仅验证窗口能创建，均不得创建公开 Release。
2. 运行 `./scripts/generate-rust-licenses.sh` 与 `node ./scripts/generate-frontend-licenses.mjs`，审阅并提交生成声明。详细范围见 `docs/third-party-license-audit.md`。
3. 用固定 sidecar 构建 DMG。`CI=true tauri build --bundles dmg` 只用于结构 smoke，会跳过 Finder 图标布局，不能作为最终发布产物；最终 DMG 必须在非 CI 环境运行 `tauri build --bundles dmg`，挂载后确认 App 位于左侧、Applications 位于右侧，两个图标在 660×400 窗口中居中并充分分隔。若 Finder 美化脚本挂起，应阻止发布并单独排查，不能回退发布左上角堆叠的 CI 产物。当前配置使用不需要 Apple Developer 账号的 ad-hoc identity `-`，发布时必须明确标注“ad-hoc 签名、未公证”，供知情测试者使用。
4. 运行 `./scripts/package-sidecar-sources-macos.sh`。进入输出目录执行 `shasum -a 256 -c Atogaki-0.1.0-third-party-sources.tar.xz.sha256`，并抽查归档的 `SOURCES.md`、`sources/SHA256SUMS` 和 `build/build-manifest.txt`。
5. 为最终 DMG 生成 SHA-256，并挂载确认 App、Applications 链接、三个 sidecar、根许可证和 `third-party/` 声明都存在。
6. 创建带版本号的 annotated tag，例如 `v0.1.0-alpha.1`，并把 tag 推送到 GitHub。
7. 在 GitHub 的 Releases 页面从该 tag 创建 prerelease，填写支持架构、macOS 版本、已知 Gatekeeper 操作、校验值、模型不内置和第三方许可证说明。
8. 上传 DMG、两个 SHA-256 文件与对应源码包，而不是把这些大产物 `git add` 到仓库。

若公开预发布在完整窗口回归前发现功能描述不成立，应立即删除 Release 及同名远端 tag，并使用新的版本号重新发布；不得替换已公开 tag 下的资产，也不要假定下载计数为零就没有外部副本。

可使用 GitHub CLI 上传已核对的产物：

```bash
gh release create v0.1.0-alpha.1 \
  path/to/Atogaki-v0.1.0-alpha.1-macos-arm64.dmg \
  path/to/Atogaki-v0.1.0-alpha.1-macos-arm64.dmg.sha256 \
  src-tauri/target/release/bundle/sources/Atogaki-0.1.0-third-party-sources.tar.xz \
  src-tauri/target/release/bundle/sources/Atogaki-0.1.0-third-party-sources.tar.xz.sha256 \
  --prerelease --title "Atogaki v0.1.0-alpha.1" --notes-file path/to/release-notes.md
```

### 自动化时机

首轮建议手工发布以稳定构建清单和窗口回归。取得 Apple Developer Program 资格后，再让 GitHub Actions 在版本 tag 上构建、签名、公证、装订 notarization ticket、生成 DMG 和校验文件，并上传到同一个 Release。签名证书、App Store Connect API key 等只放 GitHub Actions encrypted secrets，不进入仓库。

当前 Apple Silicon App 使用 ad-hoc 签名并声明最低 macOS 12.0；这可以保证 Bundle 完整性，但不能代替 Developer ID 签名与公证，也不会消除外部下载时的 Gatekeeper 提示。DMG 已在本机用 `hdiutil verify` 通过结构校验，并确认包含 `.app`、Applications 链接、三个 sidecar、Apache-2.0 项目许可证和第三方构建清单。Tauri 配置现已固定 660×400 Finder 窗口及“App 左、Applications 右”的图标位置；`CI=true` 的无美化产物仍只用于结构校验。当前 macOS 26 环境的非 CI Finder 美化脚本仍需在最终发布前实机构建并复核，自动化不能用无布局产物替代这项发布门禁。

## Windows 11 x86_64

Windows 预发布资产包括带版本名的 NSIS 安装器、相邻 SHA-256，以及包含 FFmpeg、whisper.cpp 和静态字幕栈精确源码／许可证／构建材料的 `.tar.gz` 与相邻 SHA-256。模型不进入安装包。当前安装器没有商业代码签名，只能提供给知情测试者；不得建议用户全局关闭 SmartScreen 或杀毒软件。

发布流程：

1. 从干净的候选 commit 创建并推送唯一的新预发布 tag；不得复用或移动已经公开的 tag。
2. 在 GitHub Actions 手动运行 `Windows sidecars`，选择该 tag 所在 ref，并在 `release_tag` 输入同一 tag。留空只生成 14 天 Artifact，不创建 Release。
3. 构建 job 重新生成 Windows Rust／前端许可证、复用或重建已审计 sidecar、生成 NSIS，并完成 PE GUI 子系统、安装、能力与卸载冒烟。
4. 只有构建完全通过后，独立发布 job 才取得 `contents: write` 权限。它确认 tag 精确指向本次 commit，验证安装器和对应源码的 SHA-256，从仓库读取同 tag 发布说明，然后创建 GitHub prerelease。
5. 在 Windows 11 x86_64 实机从最终 Release 重新下载并抽查哈希、SmartScreen、启动、模型、provider、识别、导出／烧录和卸载；多人扩展测试继续按 `docs/windows-testing.md` 反馈。

Windows 日常共享代码只在 PR／`main` 运行编译门禁；完整安装包不跟随普通产品提交。发布 job 拒绝不存在、指向其他 commit、格式不是 `vX.Y.Z-alpha.N` 或缺少 `docs/release-notes/<tag>.md` 的 tag，也拒绝覆盖已有 Release。
