// Régénère src/catalog.ts : pour chaque famille de scripts/families.json, demande à Google Fonts
// (css2, sans clé) quelles graisses existent. Une graisse absente fait échouer toute la feuille
// de style d'un site : le catalogue ne liste que des graisses vérifiées.
//
// Outil de maintenance (réseau requis), lancé à la main : `pnpm --filter @deep-atelier/tokens fonts`.

import { readFile, writeFile } from "node:fs/promises";

const WEIGHTS = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const here = (path) => new URL(path, import.meta.url);

async function available(family, weight) {
  const name = family.split(" ").join("+");
  const url = `https://fonts.googleapis.com/css2?family=${name}:wght@${weight}&display=swap`;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const response = await fetch(url, {
      headers: { "User-Agent": "Mozilla/5.0 (font catalogue check)" },
    });
    if (response.status === 200) return true;
    if (response.status === 400) return false;
    await new Promise((resolve) => setTimeout(resolve, 500 * (attempt + 1)));
  }
  throw new Error(`Google Fonts ne répond pas pour ${family} ${weight}`);
}

const groups = JSON.parse(await readFile(here("./families.json"), "utf8"));
const entries = [];
for (const [category, families] of Object.entries(groups)) {
  for (const family of families) {
    const checks = await Promise.all(
      WEIGHTS.map((weight) => available(family, weight)),
    );
    const weights = WEIGHTS.filter((_, index) => checks[index]);
    if (weights.length === 0) {
      throw new Error(`Famille inconnue de Google Fonts : ${family}`);
    }
    entries.push({ family, category, weights });
    console.log(`${family} (${category}) : ${weights.join(", ")}`);
  }
}

const lines = entries.map(
  ({ family, category, weights }) =>
    `  { family: ${JSON.stringify(family)}, category: ${JSON.stringify(category)}, weights: [${weights.join(", ")}] },`,
);
const source = `// Généré par scripts/probe-fonts.mjs (graisses vérifiées auprès de Google Fonts) : ne pas modifier.
import type { CatalogFont } from "./index";

export const CATALOG: readonly CatalogFont[] = [
${lines.join("\n")}
];
`;
await writeFile(here("../src/catalog.ts"), source);
console.log(`${entries.length} familles écrites dans src/catalog.ts`);
