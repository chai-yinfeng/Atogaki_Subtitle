/** User-facing notes stay separate from controls and implementation documentation. */
export const usageGuide = `<dialog id="usage-guide" class="task-sheet usage-guide" aria-labelledby="usage-title">
  <div class="sheet-heading"><h2 id="usage-title">使用说明</h2><button id="close-usage-guide" type="button" class="secondary">关闭</button></div>
  <p class="guide-intro">当前开发试用版的模型选择与使用边界。</p>
  <details open><summary>One Pass 与 Two Pass</summary>
    <p>One Pass 使用 Qwen3-ASR 1.7B 8-bit 与配套 DFlash，直接显示可修订原文。DFlash 加快解码，不是独立的识别模型。</p>
    <p>Two Pass 先显示草稿，再由 Qwen 确认。SenseVoice 周期性重新识别累积音频，并非原生流式；音频变长时会增加重算，未必比 One Pass 更快。Zipformer 是原生流式识别，当前没有已验证的日语模型。</p>
  </details>
  <details><summary>语言与正确性</summary><p>SenseVoice → Qwen 开放中文、日语、英语、韩语，不代表支持 Qwen 的全部语言。主要听审来自日语电台，其他语言没有同等质量验证。定稿表示停止修订，不保证识别或翻译正确。</p></details>
  <details><summary>VAD 与字幕打轴</summary><p>VAD 寻找人声和停顿，不理解语义，也不能提供精确的词级时间。Whisper 的离线 GGML VAD 与在线 ONNX VAD 不能互换。离线 Qwen 使用另行下载的 ForcedAligner 对齐文字与音频；它不负责修正误听。</p></details>
  <details><summary>模型取舍</summary><p>Whisper Medium 是本项目日语离线工作的常用选择。Large 在部分口语素材上更易重复，并非越大越好。Qwen 的完整、4-bit、8-bit 权重提供体积与速度取舍，运行内存可能大于文件体积。模型列表中的推荐标签不限制其他候选的使用。</p></details>
  <details><summary>芯片与本地加速</summary><p>当前聚焦 macOS Apple Silicon：Whisper 使用 whisper.cpp／Metal，Qwen 和 DFlash 使用 MLX，SenseVoice／Zipformer 草稿使用 CPU。DFlash 当前只适配配套的 Qwen 1.7B 8-bit。适配不代表所有芯片、内存和语言都已实测；Windows 暂停开发。</p></details>
  <details><summary>系统音频与数据</summary><p>在线字幕需由你授权读取系统声音，停止后结束采集，默认不保存录音。本地失败不会自动切换云端。离线识别和翻译服务分别选择；选择云端服务时注意对应的文本或音频发送范围。</p></details>
  <details><summary>当前已知限制</summary><p>持续两秒内正确译文仍是优化目标。离线 Qwen 首版在识别和对齐完成后开始翻译，新机器的 Python 运行环境自动准备尚未完成。原生编辑器首次波形绘制和更新后的音频权限保留仍在排查。</p></details>
</dialog>`;
