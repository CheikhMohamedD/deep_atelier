import { expect, test } from "@playwright/test";

import { expectNoHorizontalScroll, signIn } from "./helpers";

test("creates, opens and deletes a project", async ({ page }) => {
  await signIn(page);
  await expect(
    page.getByText("Aucun projet pour l'instant : créez le premier."),
  ).toBeVisible();

  await page.getByLabel("Nom du projet").fill("Site de bout en bout");
  await page.getByRole("button", { name: "Créer" }).click();
  await page.waitForURL(/\/projects\/[0-9a-f-]{36}$/);
  await expect(
    page.getByRole("heading", { name: "Site de bout en bout" }),
  ).toBeVisible();
  await expect(page.locator("[data-artboard]")).toHaveCount(3);
  await expectNoHorizontalScroll(page);

  await page.getByRole("link", { name: "Projets" }).click();
  await expect(
    page.getByRole("link", { name: "Site de bout en bout" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Supprimer" }).click();
  await expect(
    page.getByText("Supprimer « Site de bout en bout » ?"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Supprimer" }).click();
  await expect(
    page.getByText("Aucun projet pour l'instant : créez le premier."),
  ).toBeVisible();
});
