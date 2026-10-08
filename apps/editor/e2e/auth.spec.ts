import { expect, test } from "@playwright/test";

import { expectNoHorizontalScroll, signIn } from "./helpers";

test("protects the projects, signs in with a magic link and signs out", async ({
  page,
}) => {
  await page.goto("/projects");
  await expect(page).toHaveURL(/\/login\?next=%2Fprojects$/);
  await expect(
    page.getByRole("heading", { name: "Connexion à Deep Atelier" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Continuer avec GitHub" }),
  ).toBeVisible();
  await expectNoHorizontalScroll(page);

  await signIn(page);
  await expect(
    page.getByRole("heading", { name: "Vos projets" }),
  ).toBeVisible();
  await expectNoHorizontalScroll(page);

  await page.getByRole("button", { name: "Se déconnecter" }).click();
  await expect(page).toHaveURL(/\/login$/);
  await page.goto("/projects");
  await expect(page).toHaveURL(/\/login/);
});

test("switches the interface to English", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Langue").selectOption("en");
  await expect(
    page.getByRole("heading", { name: "Sign in to Deep Atelier" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Continue with GitHub" }),
  ).toBeVisible();
  await expectNoHorizontalScroll(page);
});
