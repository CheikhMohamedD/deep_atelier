import { expect, test } from "@playwright/test";

import {
  clickInCanvas,
  expectNoHorizontalScroll,
  frame,
  isPhone,
  openLanding,
} from "./helpers";

test("renders the page in every artboard and selects elements", async ({
  page,
}) => {
  // Sur téléphone, l'éditeur n'a que le mode Contenu et l'aperçu (voir content.spec.ts).
  test.skip(isPhone(page), "édition complète : tablette et bureau");
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
  test.skip(isPhone(page), "édition complète : tablette et bureau");
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
