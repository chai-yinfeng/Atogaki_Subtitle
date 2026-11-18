const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || "playwright");
const assert = require("node:assert/strict");
const { mkdtempSync, readFileSync, rmSync } = require("node:fs");
const { join } = require("node:path");
const { tmpdir } = require("node:os");
const { execFileSync } = require("node:child_process");
const { installEditorFixture } = require("./editor-ui-fixture.cjs");

(async () => {
  const temporary = mkdtempSync(join(tmpdir(), "atogaki-listening-video-"));
  const browser = await webkit.launch({ headless: !process.env.ATOGAKI_HEADED });
  try {
    const clip = join(temporary, "clip.mp4");
    execFileSync(process.env.ATOGAKI_TEST_FFMPEG || "ffmpeg", [
      "-v", "error", "-f", "lavfi", "-i", "color=c=navy:s=640x360:r=24:d=9",
      "-c:v", "h264_videotoolbox", "-movflags", "+faststart", clip,
    ]);
    const videoBytes = readFileSync(clip);
    const page = await browser.newPage({ viewport: { width: 1180, height: 760 } });
    const errors = [];
    page.on("pageerror", error => errors.push(String(error)));
    await installEditorFixture(page, 3, 9000);
    await page.route("**/listening-qa.mp4", async route => {
      const range = route.request().headers().range?.match(/^bytes=(\d+)-(\d*)$/);
      const start = Number(range?.[1] ?? 0);
      const end = Math.min(videoBytes.length - 1, Number(range?.[2] || videoBytes.length - 1));
      const headers = { "content-type": "video/mp4", "accept-ranges": "bytes" };
      if (range) headers["content-range"] = `bytes ${start}-${end}/${videoBytes.length}`;
      await route.fulfill({ status: range ? 206 : 200, headers, body: videoBytes.subarray(start, end + 1) });
    });
    await page.addInitScript(() => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke = async (command, args = {}) => {
        const result = await invoke(command, args);
        if (command === "list_job_summaries") return result.map(job => ({
          ...job, translation_status: "translated", translated_segment_count: job.segment_count,
        }));
        if (command === "get_playback_position") return 0;
        if (command === "get_job_detail") {
          result.job.input_path = "/qa/program.mp4";
          result.playback_path = "/listening-qa.mp4";
          result.segments[0].source_text = "原文 <B> & 記号";
          result.segments[0].translated_text = "第一条译文";
          return result;
        }
        return result;
      };
    });
    await page.goto(process.env.ATOGAKI_EDITOR_URL || "http://127.0.0.1:1430");
    await page.locator("#show-listening").click();
    await page.locator("#open-listening-library").click();
    await page.locator("[data-listen-job]").first().click();
    const video = page.locator("#listening-media-host video");
    await page.waitForFunction(() => document.querySelector("#listening-media-host video")?.readyState >= 1);
    assert.deepEqual(await video.evaluate(element => [...element.textTracks].map(track => track.mode)), ["hidden", "hidden"]);
    assert.equal(await video.evaluate(element => element.textTracks[0].cues[0].getCueAsHTML().textContent), "原文 <B> & 記号\n第一条译文");

    await video.evaluate((element, native) => {
      const button = document.createElement("button");
      button.id = "qa-fullscreen";
      button.textContent = "Full screen test";
      button.style.cssText = "position:fixed;z-index:99999;top:0;left:0";
      button.onclick = () => native ? element.webkitEnterFullscreen() : element.requestFullscreen();
      document.body.append(button);
    }, Boolean(process.env.ATOGAKI_NATIVE_VIDEO_FULLSCREEN));
    await page.locator("#qa-fullscreen").click();
    await page.waitForFunction(() => {
      const element = document.querySelector("#listening-media-host video");
      return (document.fullscreenElement === element || element?.webkitDisplayingFullscreen)
        && element.textTracks[0].mode === "showing";
    });
    await video.evaluate(element => { element.currentTime = 3.5; element.playbackRate = 1.5; });
    await page.waitForFunction(() => {
      const element = document.querySelector("#listening-media-host video");
      return element?.textTracks[0].activeCues[0]?.startTime === 3 && element.playbackRate === 1.5;
    });
    if (process.env.ATOGAKI_FULLSCREEN_SCREENSHOT) await page.screenshot({ path: process.env.ATOGAKI_FULLSCREEN_SCREENSHOT });
    await video.evaluate(element => document.fullscreenElement ? document.exitFullscreen() : element.webkitExitFullscreen());
    await page.waitForFunction(() => document.querySelector("#listening-media-host video")?.textTracks[0].mode === "hidden");
    assert.match(await page.locator("#listening-current-source").textContent(), /第 2 句/);

    const previous = await video.elementHandle();
    await page.locator("#show-workbench").click();
    assert.equal(await video.count(), 0);
    assert.equal(await previous.evaluate(element => element.isConnected), false);
    await page.locator("#show-listening").click();
    await video.waitFor();
    assert.deepEqual(await video.evaluate(element => [...element.textTracks].map(track => track.mode)), ["hidden", "hidden"]);
    assert.deepEqual(errors, []);
    console.log("PASS: video fullscreen, native bilingual track, literal cue text, seek/rate updates and release");
  } finally {
    await browser.close();
    rmSync(temporary, { recursive: true, force: true });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
