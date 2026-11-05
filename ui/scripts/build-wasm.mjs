// Builds the core library for WebAssembly (decision 0011) and copies it to
// src/generated/core.wasm, where Vite bundles it and the tests load it.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { URL, fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
execFileSync(
  "cargo",
  ["build", "-p", "scribblecap-core-wasm", "--target", "wasm32-unknown-unknown", "--release", "--locked"],
  { cwd: root, stdio: "inherit" },
);
const out = fileURLToPath(new URL("../src/generated/", import.meta.url));
mkdirSync(out, { recursive: true });
copyFileSync(`${root}/target/wasm32-unknown-unknown/release/scribblecap_core_wasm.wasm`, `${out}/core.wasm`);
