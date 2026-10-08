// Copie dans public/ ce que l'éditeur sert tel quel : le runtime de l'iframe du canvas
// (packages/canvas-runtime/dist) et le module WebAssembly du moteur (packages/engine/pkg).
// Ces fichiers sont produits par `pnpm build` (Turborepo) et ne sont pas versionnés.

import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL("..", import.meta.url));
const packages = fileURLToPath(new URL("../../../packages/", import.meta.url));
const files = [
  ["canvas-runtime/dist/index.html", "public/canvas/index.html"],
  ["canvas-runtime/dist/runtime.js", "public/canvas/runtime.js"],
  ["engine/pkg/engine_bg.wasm", "public/engine/engine_bg.wasm"],
];
const missing = files.filter(([from]) => !existsSync(`${packages}${from}`));
if (missing.length > 0) {
  console.error(
    `Fichiers absents, lancer \`pnpm build\` à la racine : ${missing.map(([from]) => from).join(", ")}`,
  );
  process.exit(1);
}
for (const [from, to] of files) {
  mkdirSync(`${here}${to.slice(0, to.lastIndexOf("/"))}`, { recursive: true });
  copyFileSync(`${packages}${from}`, `${here}${to}`);
}
