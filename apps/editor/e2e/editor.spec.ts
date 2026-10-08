import { expect, test, type Page } from "@playwright/test";

import { expectNoHorizontalScroll, seedLanding, signIn } from "./helpers";

async function openLanding(page: Page): Promise<string> {
  await signIn(page);
  const id = await seedLanding(page.context(), "Landing");
  await page.goto(`/projects/${id}`);
  await expect(page.getByRole("heading", { name: "Landing" })).toBeVisible();
  return id;
}

const frame = (page: Page, width: number) =>
  page.frameLocator(`iframe[title="Canvas, largeur ${width} px"]`);

/** Clique au centre d'un élément du canvas (sur la vitre de l'éditeur, au-dessus de l'iframe). */
async function clickInCanvas(
  page: Page,
  locator: ReturnType<ReturnType<typeof frame>["locator"]>,
  modifiers: "Shift"[] = [],
) {
  const box = await locator.boundingBox();
  if (!box) throw new Error("élément invisible");
  for (const key of modifiers) await page.keyboard.down(key);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  for (const key of modifiers) await page.keyboard.up(key);
}

test("renders the page in every artboard and selects elements", async ({
  page,
}) => {
  await openLanding(page);
  await expect(page.locator("[data-artboard]")).toHaveCount(3);
  for (const width of [390, 768, 1280]) {
    await expect(frame(page, width).locator("h1")).toContainText(
      "Dessinez votre site",
    );
  }
  await expectNoHorizontalScroll(page);

  const canvas = frame(page, 1280);
  await clickInCanvas(page, canvas.locator("h1"));
  await expect(page.getByTestId("selection-count")).toHaveText(
    "1 élément sélectionné",
  );
  await expect(
    page.locator('[data-artboard="1280"] [data-overlay="selected"]'),
  ).toHaveCount(1);
  // Le même nœud est encadré dans les autres largeurs.
  await expect(
    page.locator('[data-artboard="390"] [data-overlay="selected"]'),
  ).toHaveCount(1);

  await clickInCanvas(page, canvas.locator("main p").first(), ["Shift"]);
  await expect(page.getByTestId("selection-count")).toHaveText(
    "2 éléments sélectionnés",
  );

  await page.keyboard.press("Escape");
  await expect(page.getByTestId("selection-count")).toHaveText(
    "Aucune sélection",
  );
});

test("deletes, undoes, redoes and saves the document", async ({ page }) => {
  await openLanding(page);
  const canvas = frame(page, 1280);
  await clickInCanvas(page, canvas.locator("h1"));
  // La sélection passe par un test de position dans l'iframe : on attend qu'elle s'affiche.
  await expect(page.getByTestId("selection-count")).toHaveText(
    "1 élément sélectionné",
  );
  await page.keyboard.press("Delete");
  await expect(canvas.locator("h1")).toHaveCount(0);
  await page.keyboard.press("Control+z");
  await expect(canvas.locator("h1")).toHaveCount(1);
  await page.keyboard.press("Control+Shift+z");
  await expect(canvas.locator("h1")).toHaveCount(0);
  await expect(page.locator('[data-save="saved"]')).toBeVisible();

  await page.reload();
  await expect(page.getByRole("heading", { name: "Landing" })).toBeVisible();
  await expect(frame(page, 1280).locator("main")).toBeVisible();
  await expect(frame(page, 1280).locator("h1")).toHaveCount(0);
});

test("previews the site: the burger menu opens at 390 px", async ({ page }) => {
  await openLanding(page);
  await page.getByRole("button", { name: "Aperçu" }).click();
  await expect(page.locator('[data-testid="glass-390"]')).toHaveCount(0);
  const mobile = frame(page, 390);
  const nav = mobile.locator("nav").first();
  await expect(nav).toBeHidden();
  await mobile.getByRole("button", { name: "Menu" }).click();
  await expect(nav).toHaveAttribute("data-open", "true");
  await expect(nav).toBeVisible();
});

test("zooms the canvas", async ({ page }) => {
  await openLanding(page);
  const zoomReset = page.getByRole("button", { name: "Zoom 100 %" });
  const before = await zoomReset.textContent();
  await page.getByRole("button", { name: "Zoom avant" }).click();
  await expect(zoomReset).not.toHaveText(before ?? "");
  await zoomReset.click();
  await expect(zoomReset).toHaveText(/100/);
});
