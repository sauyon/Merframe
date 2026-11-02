import { execFileSync } from "node:child_process";
import { readdirSync, rmSync, writeFileSync } from "node:fs";

const dir = "src/types/generated";
const crates = ["wf-core", "wf-data", "wf-market", "wf-worldstate", "merframe"];

rmSync(dir, { recursive: true, force: true });
execFileSync(
  "cargo",
  [
    "test",
    "--locked",
    "-q",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    ...crates.flatMap((name) => ["-p", name]),
    "--features",
    "bindings",
    "export_bindings",
  ],
  { stdio: "inherit" },
);
const names = readdirSync(dir)
  .filter((file) => file !== "index.ts")
  .map((file) => file.slice(0, -3))
  .sort();
writeFileSync(
  `${dir}/index.ts`,
  names.map((name) => `export type * from ${JSON.stringify(`./${name}`)};\n`).join(""),
);
