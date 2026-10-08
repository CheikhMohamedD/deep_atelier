// Contrôle d'un projet exporté par Deep Atelier et servi par `next start` :
// - aucun défilement horizontal à 390, 768 et 1280 px, en thème clair et en thème sombre ;
// - aucune exception JavaScript ni erreur d'hydratation, et aucune erreur de console sauf
//   avec --allow-console-errors ;
// - avec --menu <libellé> : à 390 px, le bouton à bascule ouvre puis referme sa cible
//   (`aria-controls`, `aria-expanded`) ; à 1280 px, le bouton disparaît et la cible est visible ;
// - avec --lighthouse : Lighthouse mobile ≥ 90 dans les quatre catégories, en clair et en sombre.
//
// Usage :
//   CHROME_PATH=/usr/bin/google-chrome node check.mjs --url http://localhost:3000 \
//     --paths /,/a-propos [--menu Menu] [--lighthouse] [--allow-console-errors] [--reports dossier]
//
// --reports enregistre les captures pleine page et les rapports Lighthouse (JSON).

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import lighthouse from "lighthouse";
import puppeteer from "puppeteer-core";

const WIDTHS = [390, 768, 1280];
const SCHEMES = ["light", "dark"];
const CATEGORIES = ["performance", "accessibility", "best-practices", "seo"];
const MIN_SCORE = 0.9;

const { values: args } = parseArgs({
  options: {
    url: { type: "string" },
    paths: { type: "string", default: "/" },
    menu: { type: "string" },
    lighthouse: { type: "boolean", default: false },
    "allow-console-errors": { type: "boolean", default: false },
    reports: { type: "string" },
  },
});
if (!args.url) {
  throw new Error("--url est requis");
}
if (!process.env.CHROME_PATH) {
  throw new Error("La variable CHROME_PATH doit désigner Chrome");
}
const base = args.url.replace(/\/$/, "");
const paths = args.paths.split(",");
if (args.reports) {
  mkdirSync(args.reports, { recursive: true });
}

const failures = [];
await waitForServer(base);
const browser = await puppeteer.launch({
  executablePath: process.env.CHROME_PATH,
  args: ["--no-sandbox"],
});
try {
  for (const path of paths) {
    for (const scheme of SCHEMES) {
      for (const width of WIDTHS) {
        await checkViewport(path, scheme, width);
      }
    }
  }
  if (args.menu) {
    await checkMenu(paths[0], args.menu);
  }
  if (args.lighthouse) {
    for (const path of paths) {
      for (const scheme of SCHEMES) {
        await runLighthouse(path, scheme);
      }
    }
  }
} finally {
  await browser.close();
}
if (failures.length > 0) {
  console.error(`\n${failures.length} contrôle(s) en échec.`);
  process.exit(1);
}
console.log("\nTous les contrôles passent.");

function report(label, problems) {
  if (problems.length === 0) {
    console.log(`✓ ${label}`);
  } else {
    failures.push(label);
    console.error(`✗ ${label}\n  - ${problems.join("\n  - ")}`);
  }
}

function slug(path) {
  return path === "/" ? "accueil" : path.slice(1).replaceAll("/", "-");
}

function schemeName(scheme) {
  return scheme === "dark" ? "sombre" : "clair";
}

async function waitForServer(url) {
  const deadline = Date.now() + 120_000;
  for (;;) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch {
      // Le serveur démarre encore.
    }
    if (Date.now() > deadline) {
      throw new Error(`${url} ne répond pas`);
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}

async function openPage(path, scheme, width) {
  const page = await browser.newPage();
  const problems = [];
  page.on("pageerror", (error) =>
    problems.push(`exception : ${error.message}`),
  );
  page.on("console", (message) => {
    if (message.type() !== "error") {
      return;
    }
    const text = message.text();
    if (
      !args["allow-console-errors"] ||
      /hydrat|Minified React error/i.test(text)
    ) {
      problems.push(`console : ${text}`);
    }
  });
  await page.emulateMediaFeatures([
    { name: "prefers-color-scheme", value: scheme },
  ]);
  const mobile = width < 768;
  await page.setViewport({
    width,
    height: 900,
    isMobile: mobile,
    hasTouch: mobile,
  });
  const response = await page.goto(base + path, { waitUntil: "networkidle0" });
  // 304 : page déjà en cache, revalidée.
  if (!response || response.status() >= 400) {
    problems.push(`HTTP ${response?.status()}`);
  }
  return { page, problems };
}

async function activeScheme(page) {
  return page.evaluate(() =>
    matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
  );
}

// Débordement de la page, avec les premiers éléments qui dépassent du viewport.
async function horizontalOverflow(page) {
  return page.evaluate(() => {
    const root = document.documentElement;
    if (root.scrollWidth <= root.clientWidth) {
      return null;
    }
    const culprits = [...document.body.querySelectorAll("*")]
      .filter((el) => el.getBoundingClientRect().right > root.clientWidth + 0.5)
      .slice(0, 3)
      .map((el) => el.outerHTML.slice(0, 120));
    return `défilement horizontal : ${root.scrollWidth} px pour ${root.clientWidth} px (${culprits.join(" | ")})`;
  });
}

async function checkViewport(path, scheme, width) {
  const { page, problems } = await openPage(path, scheme, width);
  const actual = await activeScheme(page);
  if (actual !== scheme) {
    problems.push(
      `thème ${schemeName(actual)} au lieu de ${schemeName(scheme)}`,
    );
  }
  const overflow = await horizontalOverflow(page);
  if (overflow) {
    problems.push(overflow);
  }
  if (args.reports) {
    await page.screenshot({
      path: join(args.reports, `${slug(path)}-${width}-${scheme}.png`),
      fullPage: true,
    });
  }
  await page.close();
  report(`${path} à ${width} px, thème ${schemeName(scheme)}`, problems);
}

async function checkMenu(path, label) {
  const { page, problems } = await openPage(path, "light", 390);
  const selector = `button[aria-label="${label}"]`;
  const state = () =>
    page.$eval(selector, (button) => {
      const target = document.getElementById(
        button.getAttribute("aria-controls") ?? "",
      );
      return {
        expanded: button.getAttribute("aria-expanded"),
        button: getComputedStyle(button).display,
        target: target ? getComputedStyle(target).display : null,
      };
    });
  const toggle = async (expanded) => {
    await page.click(selector);
    await page
      .waitForFunction(
        (s, e) =>
          document.querySelector(s)?.getAttribute("aria-expanded") === e,
        { timeout: 5000 },
        selector,
        expanded,
      )
      .catch(() => {});
    return state();
  };

  if ((await page.$(selector)) === null) {
    problems.push(`aucun bouton ${selector}`);
  } else {
    const closed = await state();
    if (closed.target === null) {
      problems.push("aria-controls ne désigne aucun élément");
    } else if (closed.expanded !== "false" || closed.target !== "none") {
      problems.push(`fermé à 390 px : ${JSON.stringify(closed)}`);
    }
    const open = await toggle("true");
    if (open.expanded !== "true" || open.target === "none") {
      problems.push(`ouvert à 390 px : ${JSON.stringify(open)}`);
    }
    const overflow = await horizontalOverflow(page);
    if (overflow) {
      problems.push(`ouvert à 390 px, ${overflow}`);
    }
    const reclosed = await toggle("false");
    if (reclosed.expanded !== "false" || reclosed.target !== "none") {
      problems.push(`refermé à 390 px : ${JSON.stringify(reclosed)}`);
    }
    await page.setViewport({ width: 1280, height: 900 });
    const desktop = await state();
    if (desktop.button !== "none" || desktop.target === "none") {
      problems.push(`à 1280 px : ${JSON.stringify(desktop)}`);
    }
  }
  await page.close();
  report(`menu « ${label} » de ${path}`, problems);
}

async function runLighthouse(path, scheme) {
  const page = await browser.newPage();
  await page.emulateMediaFeatures([
    { name: "prefers-color-scheme", value: scheme },
  ]);
  const result = await lighthouse(
    base + path,
    { output: "json", logLevel: "error", onlyCategories: CATEGORIES },
    undefined,
    page,
  );
  const problems = [];
  const actual = await activeScheme(page);
  await page.close();
  if (actual !== scheme) {
    problems.push(
      `thème ${schemeName(actual)} au lieu de ${schemeName(scheme)}`,
    );
  }
  const lhr = result?.lhr;
  if (!lhr) {
    report(`Lighthouse ${path}, thème ${schemeName(scheme)}`, [
      "aucun résultat",
    ]);
    return;
  }
  if (lhr.runtimeError) {
    problems.push(`Lighthouse : ${lhr.runtimeError.message}`);
  }
  for (const id of CATEGORIES) {
    const category = lhr.categories[id];
    if ((category?.score ?? 0) < MIN_SCORE) {
      const audits = (category?.auditRefs ?? [])
        .filter((ref) => ref.weight > 0 && (lhr.audits[ref.id]?.score ?? 1) < 1)
        .map((ref) => ref.id);
      problems.push(
        `${id} ${Math.round((category?.score ?? 0) * 100)} < ${MIN_SCORE * 100} (${audits.join(", ")})`,
      );
    }
  }
  if (args.reports) {
    writeFileSync(
      join(args.reports, `lighthouse-${slug(path)}-${scheme}.json`),
      result.report,
    );
  }
  const scores = CATEGORIES.map(
    (id) => `${id} ${Math.round((lhr.categories[id]?.score ?? 0) * 100)}`,
  ).join(", ");
  report(
    `Lighthouse ${path}, thème ${schemeName(scheme)} : ${scores}`,
    problems,
  );
}
