import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative, resolve } from "node:path";

const root = fileURLToPath(new URL("../../", import.meta.url));
const lock = JSON.parse(readFileSync(join(root, ".github/build-inputs.json"), "utf8"));
const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"], { cwd: root, encoding: "utf8" }));
const target = resolve(metadata.target_directory);
const rel = relative(root, target);
assert(rel && !rel.startsWith("..") && !rel.includes(":"), "Native cache must be within this workspace");
const downloads = join(target, "verified-native-downloads");
const cache = join(target, ".tauri");
const assets = lock.assets.filter((asset) => asset.platform === process.platform);
assert(assets.length, `Unsupported bundle platform: ${process.platform}`);
mkdirSync(downloads, { recursive: true });
const verified = [];
for (const asset of assets) {
  const path = join(downloads, `${asset.id}`);
  if (!existsSync(path)) {
    assert(process.env.MERFRAME_NATIVE_OFFLINE !== "1", `Verified tool ${asset.file} is missing`);
    const url = `https://api.github.com/repos/${asset.repo}/releases/assets/${asset.id}`;
    const response = await fetch(url, { headers: { Accept: "application/octet-stream" }, signal: AbortSignal.timeout(120000) });
    assert(response.ok, `${asset.file}: HTTP ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    assert.equal(createHash("sha256").update(bytes).digest("hex"), asset.sha256, `${asset.file}: download checksum mismatch`);
    writeFileSync(path, bytes);
  }
  assert.equal(createHash("sha256").update(readFileSync(path)).digest("hex"), asset.sha256, `${asset.file}: cache checksum mismatch`);
  verified.push([asset, path]);
}
// Discard extracted tools from previous runs. Only verified archives enter this
// private cache; Tauri's global cache is never trusted by release builds.
rmSync(cache, { recursive: true, force: true });
mkdirSync(cache, { recursive: true });
for (const [asset, path] of verified) {
  if (asset.extract) {
    const dest = join(cache, asset.extract);
    mkdirSync(dest, { recursive: true });
    execFileSync("tar", ["-xf", path, "-C", dest], { stdio: "inherit" });
    if (asset.rename) renameSync(join(cache, asset.rename[0]), join(cache, asset.rename[1]));
  } else {
    const dest = join(cache, asset.file);
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, readFileSync(path));
    if (process.platform !== "win32") chmodSync(dest, 0o755);
  }
}
console.log(`Prepared ${verified.length} verified native bundle tools.`);
