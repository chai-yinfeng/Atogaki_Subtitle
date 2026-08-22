import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectDirectory = path.dirname(scriptDirectory);
const uiDirectory = path.join(projectDirectory, "ui");
const lockPath = path.join(uiDirectory, "package-lock.json");
const outputPath = process.argv[2]
  ? path.resolve(process.argv[2])
  : path.join(projectDirectory, "src-tauri", "third-party", "frontend-licenses.html");
const lockText = fs.readFileSync(lockPath, "utf8");
const normalizedLockText = lockText.replace(/\r\n?/g, "\n");
const lock = JSON.parse(normalizedLockText);

const acceptedExpressions = new Set([
  "Apache-2.0",
  "Apache-2.0 OR MIT",
  "BSD-3-Clause",
  "ISC",
  "MIT",
  "MPL-2.0",
]);

const normalizeText = (value) => value
  .replace(/\r\n?/g, "\n")
  .split("\n")
  .map((line) => line.trimEnd())
  .join("\n")
  .trim();

const dependencies = Object.entries(lock.packages)
  .filter(([packagePath, metadata]) => packagePath && metadata.version)
  .map(([packagePath, metadata]) => ({
    name: packagePath.replace(/^node_modules\//, ""),
    version: metadata.version,
    license: metadata.license,
    resolved: metadata.resolved,
    buildOnly: Boolean(metadata.dev),
    packagePath,
  }))
  .sort((left, right) => {
    if (left.name !== right.name) return left.name < right.name ? -1 : 1;
    if (left.version !== right.version) return left.version < right.version ? -1 : 1;
    return 0;
  });

const invalid = dependencies.filter(
  (dependency) => !dependency.license || !acceptedExpressions.has(dependency.license),
);
if (invalid.length) {
  for (const dependency of invalid) {
    console.error(`unreviewed frontend license: ${dependency.name}@${dependency.version}: ${dependency.license ?? "missing"}`);
  }
  process.exit(1);
}

const escapeHtml = (value) => String(value).replace(/[&<>"']/g, (character) => ({
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
})[character]);

const licenseDocuments = new Map();
const runtimeDependencies = dependencies.filter((dependency) => !dependency.buildOnly);
for (const dependency of runtimeDependencies) {
  const installedDirectory = path.join(uiDirectory, dependency.packagePath);
  if (!fs.existsSync(installedDirectory)) continue;
  const licenseFiles = fs.readdirSync(installedDirectory)
    .filter((name) => /^(license|copying|notice)/i.test(name))
    .sort();
  for (const fileName of licenseFiles) {
    const text = normalizeText(fs.readFileSync(path.join(installedDirectory, fileName), "utf8"));
    const digest = crypto.createHash("sha256").update(text).digest("hex");
    const document = licenseDocuments.get(digest) ?? { text, usedBy: [] };
    document.usedBy.push(`${dependency.name}@${dependency.version} (${fileName})`);
    licenseDocuments.set(digest, document);
  }
}

for (const dependency of runtimeDependencies) {
  const hasText = [...licenseDocuments.values()].some((document) =>
    document.usedBy.some((usedBy) => usedBy.startsWith(`${dependency.name}@${dependency.version} `)),
  );
  if (!hasText) {
    console.error(`runtime frontend dependency has no installed license text: ${dependency.name}@${dependency.version}`);
    process.exit(1);
  }
}

const lockDigest = crypto.createHash("sha256").update(normalizedLockText).digest("hex");
const rows = dependencies.map((dependency) => {
  const source = dependency.resolved ?? `https://www.npmjs.com/package/${dependency.name}/v/${dependency.version}`;
  return `<tr><td><a href="${escapeHtml(source)}">${escapeHtml(dependency.name)}</a></td><td>${escapeHtml(dependency.version)}</td><td>${escapeHtml(dependency.license)}</td><td>${dependency.buildOnly ? "build" : "runtime"}</td></tr>`;
}).join("\n");
const documents = [...licenseDocuments.values()].map((document, index) => `
<section>
  <h2>License text ${index + 1}</h2>
  <p>${document.usedBy.map(escapeHtml).join(", ")}</p>
  <pre>${escapeHtml(document.text)}</pre>
</section>`).join("\n");

const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Atogaki frontend third-party licenses</title>
  <style>body{font:14px/1.5 system-ui,sans-serif;max-width:1100px;margin:32px auto;padding:0 20px;color:#222}table{border-collapse:collapse;width:100%}th,td{border:1px solid #ccc;padding:6px;text-align:left}pre{white-space:pre-wrap;background:#f5f5f5;padding:12px;overflow:auto}</style>
</head>
<body>
  <h1>Atogaki frontend third-party licenses</h1>
  <p>Generated from ui/package-lock.json. Runtime dependencies are shipped in the WebView bundle and include their installed license text below. Build dependencies are recorded by package, version, license expression and source for audit completeness, but are not copied into the App.</p>
  <p>Lockfile SHA-256: <code>${lockDigest}</code>. Runtime packages: ${runtimeDependencies.length}. Build packages: ${dependencies.length - runtimeDependencies.length}.</p>
  <table><thead><tr><th>Package</th><th>Version</th><th>License</th><th>Scope</th></tr></thead><tbody>${rows}</tbody></table>
${documents}
</body>
</html>
`;

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, html);
console.log(`Generated ${outputPath}`);
console.log(`Audited ${runtimeDependencies.length} runtime and ${dependencies.length - runtimeDependencies.length} build dependencies.`);
