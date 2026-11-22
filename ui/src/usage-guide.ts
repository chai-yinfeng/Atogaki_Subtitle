/** User-facing notes stay separate from controls and implementation documentation. */
export const usageGuide = `<dialog id="usage-guide" class="task-sheet usage-guide" aria-labelledby="usage-title">
  <div class="sheet-heading"><h2 id="usage-title">使用说明</h2><button id="close-usage-guide" type="button" class="secondary">关闭</button></div>
  <p class="guide-intro">了解如何看字幕、查词、复习例句，以及使用在线字幕。</p>
  <details open><summary>用日语字幕学习</summary>
    <p><strong>准备词典。</strong> 在「设置 → 词典」下载并安装「Sudachi 日语分词」。需要中文释义时安装 Tomoshi，需要英文释义时安装 JMdict。安装后，查词和句型分析都可以离线使用。</p>
    <p><strong>查词和看词形。</strong> 在「收听」打开日语节目，点击或框选日语原文。点一个词查看词典形、读音和释义。遇到活用时，「本句词形」会说明能确认的词形变化。选好释义后点「收藏」。</p>
    <p><strong>分析句型。</strong> 点击字幕旁的「分析句型」，也可以框选日语原文后再分析。句子分在相邻两行时，可以一起框选。结果会高亮匹配到的部分，并显示「本句用法」。点「收藏语法例句」可以保存原句。遇到暂未识别的表达，可打开「学习 → 日语语法」，按句型或等级查阅中文目录。</p>
    <p><strong>复习和记录疑问。</strong> 在「学习」点「播放例句」，即可回到原节目。发现句型有遗漏或识别有误时，可以在分析结果里点「标记漏报」或「标记误报」，之后在学习区搜索「反馈」回看。反馈保存在这台设备。</p>
  </details>
  <details><summary>One Pass 与 Two Pass</summary>
    <p>One Pass 直接显示识别结果，后面的声音可能让字幕继续修订。Two Pass 先显示草稿，再用第二次识别确认文字。字幕标为定稿后会停止修订，重要内容仍可听原音确认。</p>
    <p>请先选择节目语言，再从可用模型中选择。模型未就绪时，可按界面提示准备所需的下载项。字幕出现的速度会随设备、模型和音频变化。</p>
  </details>
  <details><summary>离线转录与时间轴</summary>
    <p>可在「设置 → 识别」选择模型。VAD 用来寻找人声和停顿，帮助切分字幕。用 Qwen 转录时，需要按提示准备对齐模型，让识别文字跟上音频。任务完成后可在字幕编辑器修改原文和时间。</p>
    <p>模型文件需要一定的存储空间，运行时还会占用内存。遇到内存不足或处理过慢时，可尝试较小的模型。</p>
  </details>
  <details><summary>系统音频与数据</summary>
    <p>在线字幕需要系统音频录制权限。停止在线字幕后，App 会结束采集，默认不保存录音。选择云端服务前，可以查看设置里说明的音频或字幕发送范围。</p>
  </details>
  <details><summary>App 更新与系统音频授权</summary>
    <p>支持应用内更新的版本可在「设置 → 更新」检查更新。安装前请保存编辑、停止在线字幕，并等后台任务完成。安装完成后 App 会重新打开。</p>
    <p>更新后，系统可能再次询问是否允许录制系统音频。需要继续使用在线字幕时，可以按系统提示授权。</p>
  </details>
</dialog>`;
