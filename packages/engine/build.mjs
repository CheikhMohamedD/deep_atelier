// Construit le moteur WebAssembly : crates/engine-wasm → packages/engine/pkg (wasm-bindgen,
// cible `web`). wasm-bindgen-cli doit avoir exactement la version de la crate wasm-bindgen.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const run = (command, args) =>
  execFileSync(command, args, { cwd: root, stdio: "inherit" });

const manifest = readFileSync(`${root}crates/engine-wasm/Cargo.toml`, "utf8");
const expected = /wasm-bindgen = "=([^"]+)"/.exec(manifest)?.[1];
let installed;
try {
  installed = execFileSync("wasm-bindgen", ["--version"], { encoding: "utf8" })
    .trim()
    .split(" ")
    .pop();
} catch {
  installed = undefined;
}
if (!expected || installed !== expected) {
  console.error(
    `wasm-bindgen-cli ${expected} requis (trouvé : ${installed ?? "aucun"}). ` +
      `Installer : cargo install wasm-bindgen-cli --version ${expected} --locked`,
  );
  process.exit(1);
}

run("cargo", [
  "build",
  "--profile",
  "wasm",
  "--target",
  "wasm32-unknown-unknown",
  "-p",
  "deep-atelier-engine-wasm",
]);
run("wasm-bindgen", [
  "--target",
  "web",
  "--out-dir",
  "packages/engine/pkg",
  "--out-name",
  "engine",
  "target/wasm32-unknown-unknown/wasm/engine.wasm",
]);
