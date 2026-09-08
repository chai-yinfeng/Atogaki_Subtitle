import { readFile } from "node:fs/promises";

const configUrl = new URL("../src-tauri/tauri.windows.conf.json", import.meta.url);
const config = JSON.parse(await readFile(configUrl, "utf8"));
const bundle = config.bundle ?? {};
const nsis = bundle.windows?.nsis ?? {};

function requireCondition(condition, message) {
  if (!condition) throw new Error(`invalid Windows Tauri config: ${message}`);
}

requireCondition(bundle.targets?.includes("nsis"), "bundle.targets must include nsis");
requireCondition(bundle.icon?.includes("icons/icon.ico"), "bundle.icon must include icons/icon.ico");
requireCondition(nsis.installMode === "currentUser", "NSIS installMode must be currentUser");
requireCondition(
  Array.isArray(nsis.languages)
    && nsis.languages.includes("English")
    && nsis.languages.includes("SimpChinese"),
  "NSIS languages must include English and SimpChinese",
);

console.log("Windows Tauri NSIS configuration verified");
