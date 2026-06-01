import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

function readAppVersion() {
  const configPath = path.resolve("apps/codex-manager/src-tauri/tauri.conf.json");
  const config = JSON.parse(readFileSync(configPath, "utf8"));
  return String(config.version || "").trim();
}

function trimSlashes(value) {
  return String(value || "").replace(/^\/+|\/+$/g, "");
}

function encodePathSegment(value) {
  return encodeURIComponent(value).replace(/[!'()*]/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);
}

const releaseTag =
  process.env.RELEASE_TAG ||
  process.env.GITHUB_REF_NAME ||
  `v${readAppVersion()}`;
const repository = process.env.GITHUB_REPOSITORY || "mocheng998/codex-manager";
const downloadBaseUrl = String(process.env.CLOUDFLARE_DOWNLOAD_BASE_URL || "").replace(/\/+$/g, "");
const pathPrefix = trimSlashes(process.env.CLOUDFLARE_DOWNLOAD_PATH_PREFIX || "codex-manager");
const outputPath = process.env.LATEST_JSON_PATH || "target/release/latest.json";
const files = process.argv.slice(2).filter((file) => existsSync(file));

if (files.length === 0) {
  throw new Error("No release assets were found for latest.json");
}

function assetUrl(file) {
  const name = path.basename(file);
  if (downloadBaseUrl) {
    const segments = [pathPrefix, "releases", releaseTag, name].filter(Boolean).map(encodePathSegment);
    return `${downloadBaseUrl}/${segments.join("/")}`;
  }
  return `https://github.com/${repository}/releases/latest/download/${encodePathSegment(name)}`;
}

const manifestUrl = downloadBaseUrl
  ? `${downloadBaseUrl}/${[pathPrefix, "latest.json"].filter(Boolean).map(encodePathSegment).join("/")}`
  : `https://github.com/${repository}/releases/tag/${releaseTag}`;

const manifest = {
  version: releaseTag,
  url: manifestUrl,
  notes: `Automated build for ${releaseTag}`,
  assets: files.map((file) => ({
    name: path.basename(file),
    url: assetUrl(file),
  })),
};

mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`Wrote ${outputPath}`);
