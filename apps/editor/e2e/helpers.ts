import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expect, type BrowserContext, type Page } from "@playwright/test";

const MAILPIT_URL = process.env.MAILPIT_URL ?? "http://127.0.0.1:54324";
const API_URL = process.env.E2E_API_URL ?? "http://127.0.0.1:3001";

/** Landing de démonstration (compiler_web::demo::landing), écrite par global-setup. */
export const LANDING_FIXTURE =
  process.env.E2E_LANDING ??
  fileURLToPath(new URL("../../../target/e2e/landing.json", import.meta.url));

export function uniqueEmail(prefix: string): string {
  const suffix = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
  return `${prefix}-${suffix}@example.com`;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

type MailpitList = { messages?: { ID: string }[] };
type MailpitMessage = { HTML?: string; Text?: string };

/** Lien de connexion reçu par `email` dans Mailpit (stack Supabase local). */
async function magicLink(email: string): Promise<string> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const response = await fetch(
      `${MAILPIT_URL}/api/v1/search?query=${encodeURIComponent(`to:"${email}"`)}`,
    );
    const list = (await response.json()) as MailpitList;
    const id = list.messages?.[0]?.ID;
    if (id) {
      const message = (await (
        await fetch(`${MAILPIT_URL}/api/v1/message/${id}`)
      ).json()) as MailpitMessage;
      const body = `${message.HTML ?? ""} ${message.Text ?? ""}`;
      const link = /https?:\/\/[^\s"'<>]+\/auth\/v1\/verify[^\s"'<>]*/.exec(
        body,
      )?.[0];
      if (link) return link.replaceAll("&amp;", "&");
    }
    await sleep(250);
  }
  throw new Error(`Aucun lien de connexion reçu pour ${email}`);
}

/** Connexion par lien magique ; renvoie l'adresse utilisée. */
export async function signIn(
  page: Page,
  email = uniqueEmail("e2e"),
): Promise<string> {
  await page.goto("/login");
  await page.getByLabel("Adresse e-mail").fill(email);
  await page
    .getByRole("button", { name: "Recevoir un lien de connexion" })
    .click();
  await expect(page.getByRole("status")).toContainText(email);
  await page.goto(await magicLink(email));
  await page.waitForURL(/\/projects$/);
  return email;
}

/** Jeton d'accès de la session, lu dans les cookies de @supabase/ssr. */
export async function accessToken(context: BrowserContext): Promise<string> {
  const chunks = (await context.cookies())
    .filter((cookie) => /^sb-.+-auth-token(\.\d+)?$/.test(cookie.name))
    .sort((a, b) => a.name.localeCompare(b.name, "en", { numeric: true }));
  const raw = chunks.map((cookie) => cookie.value).join("");
  const json = raw.startsWith("base64-")
    ? Buffer.from(raw.slice("base64-".length), "base64url").toString("utf8")
    : decodeURIComponent(raw);
  const token = (JSON.parse(json) as { access_token?: string }).access_token;
  if (!token) throw new Error("Pas de session dans les cookies");
  return token;
}

/** Projet créé par l'API et peuplé avec la landing de démonstration ; renvoie son id. */
export async function seedLanding(
  context: BrowserContext,
  name: string,
): Promise<string> {
  const headers = {
    Authorization: `Bearer ${await accessToken(context)}`,
    "Content-Type": "application/json",
  };
  const created = (await (
    await fetch(`${API_URL}/projects`, {
      method: "POST",
      headers,
      body: JSON.stringify({ name }),
    })
  ).json()) as { project: { id: string } };
  const document: unknown = JSON.parse(await readFile(LANDING_FIXTURE, "utf8"));
  const saved = await fetch(
    `${API_URL}/projects/${created.project.id}/document`,
    {
      method: "PUT",
      headers,
      body: JSON.stringify({ base_version: 1, document }),
    },
  );
  expect(saved.ok, await saved.clone().text()).toBe(true);
  return created.project.id;
}

/** La page ne défile pas horizontalement. */
export async function expectNoHorizontalScroll(page: Page): Promise<void> {
  const overflow = await page.evaluate(
    () =>
      document.documentElement.scrollWidth -
      document.documentElement.clientWidth,
  );
  expect(overflow).toBeLessThanOrEqual(0);
}

/** Téléphone : sous 768 px, l'éditeur n'offre que le mode Contenu et l'aperçu. */
export function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 0) < 768;
}

/** Sous 1024 px, les panneaux sont des feuilles ouvertes par un bouton de la barre du haut. */
export function isCompact(page: Page): boolean {
  return (page.viewportSize()?.width ?? 0) < 1024;
}

/** Connexion, projet peuplé avec la landing de démonstration, éditeur ouvert. */
export async function openLanding(page: Page): Promise<string> {
  await signIn(page);
  const id = await seedLanding(page.context(), "Landing");
  await page.goto(`/projects/${id}`);
  await expect(page.getByRole("heading", { name: "Landing" })).toBeVisible();
  return id;
}

export const frame = (page: Page, width: number) =>
  page.frameLocator(`iframe[title="Canvas, largeur ${width} px"]`);

type CanvasLocator = ReturnType<ReturnType<typeof frame>["locator"]>;

/** Clique au centre d'un élément du canvas (sur la vitre de l'éditeur, au-dessus de l'iframe). */
export async function clickInCanvas(
  page: Page,
  locator: CanvasLocator,
  modifiers: "Shift"[] = [],
): Promise<void> {
  const box = await locator.boundingBox();
  if (!box) throw new Error("élément invisible");
  for (const key of modifiers) await page.keyboard.down(key);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  for (const key of modifiers) await page.keyboard.up(key);
}

/** Ouvre le panneau gauche sur un onglet (une feuille sous 1024 px). */
export async function openLeftPanel(
  page: Page,
  tab: "Calques" | "Pages" | "Tokens",
): Promise<void> {
  const panel = page.locator('[data-panel="left"]');
  if (isCompact(page) && !(await panel.isVisible())) {
    await page
      .getByRole("button", { name: "Calques, pages et tokens" })
      .click();
  }
  await page.getByRole("tab", { name: tab }).click();
  await expect(panel).toBeVisible();
}

/** Ouvre l'inspecteur (une feuille sous 1024 px) et le renvoie. */
export async function openInspector(page: Page, name = "Inspecteur") {
  const panel = page.locator('[data-panel="inspector"]');
  if (isCompact(page) && !(await panel.isVisible())) {
    await page.getByRole("button", { name, exact: true }).click();
  }
  await expect(panel).toBeVisible();
  return panel;
}

/** Ferme la feuille ouverte sous 1024 px, pour retrouver tout le canvas. */
export async function closeSheet(page: Page): Promise<void> {
  if (!isCompact(page)) return;
  const close = page.getByRole("button", { name: /^Fermer : / });
  if (await close.first().isVisible()) await close.first().click();
}
