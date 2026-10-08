// Bundle de l'iframe du canvas : un seul script classique (IIFE), car l'iframe isolée a une
// origine opaque et un module ES y demanderait des en-têtes CORS.

import { copyFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const here = fileURLToPath(new URL(".", import.meta.url));
mkdirSync(`${here}dist`, { recursive: true });
await build({
  entryPoints: [`${here}src/main.tsx`],
  outfile: `${here}dist/runtime.js`,
  bundle: true,
  format: "iife",
  platform: "browser",
  target: "es2022",
  minify: true,
  legalComments: "external",
  define: { "process.env.NODE_ENV": '"production"' },
  logLevel: "warning",
});
copyFileSync(`${here}index.html`, `${here}dist/index.html`);
