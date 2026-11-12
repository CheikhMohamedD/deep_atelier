// Mode Contenu : textes, images et liens seulement. C'est le seul mode d'édition sur téléphone
// (ADR 0001 § 12, question 12).

import { expect, test } from "@playwright/test";

import {
  clickInCanvas,
  closeSheet,
  expectNoHorizontalScroll,
  frame,
  isPhone,
  openLanding,
} from "./helpers";

test("content mode edits texts and links, and nothing else", async ({
  page,
}) => {
  await openLanding(page);
  const phone = isPhone(page);
  if (phone) {
    // Téléphone : ni mode Design, ni calques, ni largeurs ; un seul artboard de 390 px.
    await expect(page.getByRole("button", { name: "Design" })).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Calques, pages et tokens" }),
    ).toHaveCount(0);
    await expect(page.getByLabel("Breakpoint édité")).toHaveCount(0);
    await expect(page.locator("[data-artboard]")).toHaveCount(1);
    await expect(page.locator('[data-artboard="390"]')).toBeVisible();
    await expect(page.getByRole("button", { name: "Contenu" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  } else {
    await page.getByRole("button", { name: "Contenu" }).click();
    await expect(page.locator('[data-panel="left"]')).toHaveCount(0);
  }
  await expectNoHorizontalScroll(page);

  // Toucher un texte ouvre ses champs.
  const canvas = frame(page, 390);
  await clickInCanvas(page, canvas.locator("h1"));
  const panel = page.locator('[data-panel="inspector"]');
  await expect(panel).toBeVisible();
  await expect(panel.locator("[data-prop]")).toHaveCount(0);
  await panel.getByLabel("Texte", { exact: true }).fill("Bonjour Deep Atelier");
  await page.keyboard.press("Control+Enter");
  await expect(canvas.locator("h1")).toHaveText("Bonjour Deep Atelier");

  // Supprimer ne fait rien en mode Contenu.
  await panel.getByText("Contenu", { exact: true }).first().click();
  await page.keyboard.press("Delete");
  await expect(canvas.locator("h1")).toHaveCount(1);

  // Un lien : libellé et destination.
  await closeSheet(page);
  await clickInCanvas(
    page,
    canvas.getByRole("link", { name: "Commencer gratuitement" }),
  );
  await expect(panel).toBeVisible();
  await panel.getByLabel("Libellé").fill("Essayer");
  await page.keyboard.press("Enter");
  await expect(canvas.getByRole("link", { name: "Essayer" })).toBeVisible();
  await panel.getByLabel("Destination").selectOption({ label: "E-mail" });
  await panel.getByLabel("Adresse e-mail").fill("bonjour@example.com");
  await page.keyboard.press("Enter");
  await expect(canvas.getByRole("link", { name: "Essayer" })).toHaveAttribute(
    "href",
    "mailto:bonjour@example.com",
  );

  // Une image : texte alternatif.
  await closeSheet(page);
  await clickInCanvas(page, canvas.locator("main img").first());
  await expect(panel).toBeVisible();
  await panel.getByLabel("Texte alternatif").fill("Capture de l'éditeur");
  await page.keyboard.press("Enter");
  await expect(canvas.locator("main img").first()).toHaveAttribute(
    "alt",
    "Capture de l'éditeur",
  );

  await expect(page.locator('[data-save="saved"]')).toBeVisible();
  await expectNoHorizontalScroll(page);

  // L'aperçu reste disponible, sans champs d'édition.
  await page.getByRole("button", { name: "Aperçu" }).click();
  await expect(panel).toHaveCount(0);
  await expect(canvas.locator("h1")).toHaveText("Bonjour Deep Atelier");
});
