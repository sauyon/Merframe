import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";

const root = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");
const exact = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const vendored = JSON.parse(read("vendor/upstream.json"));
for (const crate of vendored) {
  assert(/^[a-f0-9]{64}$/.test(crate.sha256), `${crate.name}: missing upstream archive hash`);
  assert.equal(
    createHash("sha256").update(read(crate.patch).replace(/\r\n/g, "\n")).digest("hex"),
    crate.patch_sha256,
    `${crate.name}: reviewed patch changed`,
  );
  const walk = (dir, prefix = "") => readdirSync(dir).flatMap((name) => {
    const path = new URL(`${name}`, dir);
    assert(!lstatSync(path).isSymbolicLink(), "Vendor symlinks are not allowed");
    return lstatSync(path).isDirectory()
      ? walk(new URL(`${name}/`, dir), `${prefix}${name}/`)
      : [`${prefix}${name}`];
  });
  const directory = new URL(`${crate.path}/`, root);
  assert.deepEqual(walk(directory).sort(), Object.keys(crate.files).sort(), `${crate.name}: vendor file list changed`);
  for (const [path, expected] of Object.entries(crate.files)) {
    const bytes = readFileSync(new URL(path, directory));
    const content = expected.text ? bytes.toString("utf8").replace(/\r\n/g, "\n") : bytes;
    assert.equal(createHash("sha256").update(content).digest("hex"), expected.sha256, `${crate.name}/${path}: reviewed source hash changed`);
  }
}
const pkg = JSON.parse(read("app/package.json"));
const lock = read("app/pnpm-lock.yaml");
const importer = lock.split("\npackages:")[0];

let npmCount = 0;
for (const group of ["dependencies", "devDependencies", "optionalDependencies"]) {
  for (const [name, version] of Object.entries(pkg[group] ?? {})) {
    assert(exact.test(version), `${name}: expected an exact npm version`);
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const entry = importer.match(
      new RegExp(
        `^      (?:'${escaped}'|${escaped}):\\r?\\n        specifier: ([^\\r\\n]+)\\r?\\n        version: ([^\\r\\n(]+)`,
        "m",
      ),
    );
    assert(entry, `${name}: missing from pnpm lockfile`);
    assert.equal(entry[1], version, `${name}: stale lockfile specifier`);
    assert.equal(entry[2], version, `${name}: different locked version`);
    npmCount++;
  }
}

const metadata = JSON.parse(
  execFileSync(
    "cargo",
    ["metadata", "--locked", "--no-deps", "--format-version", "1"],
    { cwd: root, encoding: "utf8" },
  ),
);
let rustCount = 0;
for (const member of metadata.packages) {
  for (const dep of member.dependencies) {
    if (dep.path) {
      continue;
    }
    assert(
      dep.source?.startsWith("registry+") &&
        dep.req.startsWith("=") &&
        exact.test(dep.req.slice(1)),
      `${member.name}/${dep.name}: expected an exact registry version`,
    );
    rustCount++;
  }
}

const workflow = read(".github/workflows/release.yml");
for (const [, action, ref] of workflow.matchAll(/uses:\s*([^\s@]+)@([^\s#]+)/g)) {
  assert(/^[a-f0-9]{40}$/.test(ref), `${action}: expected a full commit SHA`);
}
for (const [, container] of workflow.matchAll(/container:\s*(\S+)/g)) {
  assert(/@sha256:[a-f0-9]{64}$/.test(container), "Expected a container digest");
}
assert(!workflow.includes("windows-latest"), "Use an explicit Windows runner release");
assert(!workflow.includes("apt-get install"), "Linux builds must use the pinned builder image");
assert(!workflow.includes("pacman -Syu "), "Use the immutable Arch snapshot");
const buildInputs = JSON.parse(read(".github/build-inputs.json"));
for (const asset of buildInputs.assets) {
  assert(Number.isSafeInteger(asset.id) && /^[a-f0-9]{64}$/.test(asset.sha256), `Invalid native tool pin: ${asset.file}`);
}
assert.match(read("packaging/build/Dockerfile"), /^FROM [^\n]+@sha256:[a-f0-9]{64}$/m);
assert(read("packaging/build/Dockerfile").includes(buildInputs.ubuntuSnapshot), "Ubuntu snapshot differs from build-input lock");
assert(workflow.includes(buildInputs.archSnapshot), "Arch snapshot differs from build-input lock");
assert(exact.test(read(".node-version").trim()), "Expected an exact Node version");
assert(
  /^pnpm@\d+\.\d+\.\d+$/.test(pkg.packageManager),
  "Expected an exact pnpm version",
);
assert(
  /channel = "\d+\.\d+\.\d+"/.test(read("rust-toolchain.toml")),
  "Expected an exact Rust toolchain",
);
const builder = read("packaging/build/Dockerfile");
const rustVersion = read("rust-toolchain.toml").match(/channel = "([^"]+)"/)[1];
assert(builder.includes(`rust-${rustVersion}-x86_64-unknown-linux-gnu.tar.xz`), "Builder Rust version differs from the project pin");
assert(builder.includes(`node-v${read(".node-version").trim()}-linux-x64.tar.xz`), "Builder Node version differs from the project pin");
assert(builder.includes(`pnpm-${pkg.packageManager.slice(5)}.tgz`), "Builder pnpm version differs from the project pin");
console.log(
  `Verified ${npmCount} npm pins, ${rustCount} Rust declarations, ${vendored.length} source backports, CI action SHAs and toolchain pins.`,
);
