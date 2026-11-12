// Étape (d) : breakpoint actif, calques, inspecteur, pages et tokens. Édition complète, donc
// tablette et bureau ; le téléphone a son propre scénario (content.spec.ts).

import { expect, test } from "@playwright/test";

import {
  clickInCanvas,
  closeSheet,
  expectNoHorizontalScroll,
  frame,
  isPhone,
  openInspector,
  openLanding,
  openLeftPanel,
} from "./helpers";

test.beforeEach(({ page }) => {
  test.skip(isPhone(page), "édition complète : tablette et bureau");
});

test("the inspector edits the breakpoint of the clicked artboard", async ({
  page,
}) => {
  await openLanding(page);
  const breakpoint = page.getByLabel("Breakpoint édité");
  await expect(breakpoint).toHaveValue("base");
  await expect(page.locator('[data-artboard="390"]')).toHaveAttribute(
    "data-active",
    "true",
  );

  // Cliquer dans l'artboard de 768 px : l'inspecteur édite `md`.
  await clickInCanvas(page, frame(page, 768).locator("h1"));
  await expect(breakpoint).toHaveValue("md");
  await expect(page.locator('[data-artboard="768"]')).toHaveAttribute(
    "data-active",
    "true",
  );
  await expect(page.locator('[data-artboard="390"]')).toHaveAttribute(
    "data-active",
    "false",
  );

  const inspector = await openInspector(page);
  const align = inspector.locator('[data-prop="text_align"]');
  await align.getByLabel("Alignement du texte").selectOption("end");
  // Surcharge à `md` : pastille, classe produite, et le rendu suit à partir de 768 px.
  await expect(align.locator('[data-origin="set"]')).toContainText("md");
  await expect(align.locator('[data-class="md:text-end"]')).toBeVisible();
  await expect(frame(page, 768).locator("h1")).toHaveClass(/md:text-end/);
  await expect(frame(page, 768).locator("h1")).toHaveCSS("text-align", "end");
  await expect(frame(page, 1280).locator("h1")).toHaveCSS("text-align", "end");
  await expect(frame(page, 390).locator("h1")).not.toHaveCSS(
    "text-align",
    "end",
  );
  await expectNoHorizontalScroll(page);

  // Le sélecteur change de breakpoint : en `base`, la valeur neutre est posée, sans classe `md:`.
  await breakpoint.selectOption("base");
  await expect(page.locator('[data-artboard="390"]')).toHaveAttribute(
    "data-active",
    "true",
  );
  await expect(align.locator('[data-origin="set"]')).toContainText("base");
  await expect(align.locator('[data-class="md:text-end"]')).toHaveCount(0);
  await expect(align).toContainText("surchargé en md");

  // Un breakpoint sans artboard l'affiche ; la valeur y est héritée de `md`.
  await breakpoint.selectOption("lg");
  await expect(page.locator('[data-artboard="1024"]')).toHaveAttribute(
    "data-active",
    "true",
  );
  await expect(align.locator('[data-origin="inherited"]')).toHaveText(
    "hérité de md",
  );

  // Réinitialiser à l'héritage : la surcharge `md` disparaît.
  await breakpoint.selectOption("md");
  await align
    .getByRole("button", { name: /^Réinitialiser à l'héritage/ })
    .click();
  await expect(frame(page, 768).locator("h1")).not.toHaveClass(/md:text-end/);
  await expect(align.locator('[data-origin="inherited"]')).toHaveText(
    "hérité de base",
  );
  await expect(page.locator('[data-save="saved"]')).toBeVisible();
});

test("layers follow the canvas: select, rename, hide, lock, reorder", async ({
  page,
}) => {
  await openLanding(page);
  await openLeftPanel(page, "Calques");
  const layers = page.locator('[data-panel="left"]');
  await expect(
    layers.getByRole("tree", { name: "Layout · site" }),
  ).toBeVisible();
  await expect(
    layers.getByRole("tree", { name: "Page · Accueil" }),
  ).toBeVisible();

  // Choisir un calque le sélectionne dans le canvas.
  const title = layers.getByRole("treeitem", {
    name: /^Dessinez votre site/,
  });
  await title.getByRole("button", { name: /^Dessinez votre site/ }).click();
  await expect(title).toHaveAttribute("aria-selected", "true");
  await expect(page.getByTestId("selection-count")).toHaveText(
    "1 élément sélectionné",
  );
  await expect(
    page.locator('[data-artboard="390"] [data-overlay="selected"]'),
  ).toHaveCount(1);

  // Renommer.
  await title.getByRole("button", { name: /^Dessinez votre site/ }).dblclick();
  await layers.getByLabel("Nom du calque").fill("Titre principal");
  await page.keyboard.press("Enter");
  const renamed = layers.getByRole("treeitem", { name: "Titre principal" });
  await expect(renamed).toBeVisible();

  // Masquer, puis afficher.
  await renamed
    .getByRole("button", { name: "Masquer Titre principal" })
    .click();
  await expect(frame(page, 390).locator("h1")).toBeHidden();
  await renamed
    .getByRole("button", { name: "Afficher Titre principal" })
    .click();
  await expect(frame(page, 390).locator("h1")).toBeVisible();

  // Descendre d'un rang : le titre passe après le paragraphe.
  const order = async () =>
    frame(page, 390)
      .locator("h1")
      .evaluate((h1) => {
        const siblings = [...(h1.parentElement?.children ?? [])];
        return siblings.indexOf(h1);
      });
  expect(await order()).toBe(0);
  await renamed
    .getByRole("button", { name: "Descendre Titre principal" })
    .click();
  await expect.poll(order).toBe(1);
  await renamed.getByRole("button", { name: "Monter Titre principal" }).click();
  await expect.poll(order).toBe(0);

  // Verrouiller : le calque ne se supprime plus, et un clic dans le canvas ne le prend plus.
  await renamed
    .getByRole("button", { name: "Verrouiller Titre principal" })
    .click();
  await renamed
    .getByRole("button", { name: "Titre principal" })
    .first()
    .click();
  await page.keyboard.press("Delete");
  await expect(frame(page, 390).locator("h1")).toHaveCount(1);
  await closeSheet(page);
  await clickInCanvas(page, frame(page, 390).locator("h1"));
  await expect(page.getByTestId("selection-count")).toHaveText(
    "1 élément sélectionné",
  );
  const inspector = await openInspector(page);
  await expect(inspector.getByTestId("inspector")).not.toContainText(
    "Calque verrouillé",
  );
  await expectNoHorizontalScroll(page);
});

test("pages: create, route, SEO, layout, and the document is saved", async ({
  page,
}) => {
  await openLanding(page);
  await openLeftPanel(page, "Pages");
  const panel = page.locator('[data-panel="left"]');
  await panel.getByLabel("Nouvelle page").fill("Nos tarifs");
  await panel.getByRole("button", { name: "Ajouter" }).first().click();

  // La page créée est affichée, avec une route proposée d'après son nom.
  const current = panel.locator('[aria-current="page"]');
  await expect(current).toContainText("Nos tarifs");
  await expect(current).toContainText("/nos-tarifs");
  await expect(panel.getByLabel("Route")).toHaveValue("/nos-tarifs");

  // Route dynamique simple.
  await panel.getByLabel("Route").fill("/offres/[plan]");
  await page.keyboard.press("Enter");
  await expect(current).toContainText("/offres/[plan]");
  // Une route mal formée est signalée et n'est pas appliquée.
  await panel.getByLabel("Route").fill("/offres/[plan");
  await page.keyboard.press("Enter");
  await expect(panel.getByText("Route mal formée")).toBeVisible();
  await expect(current).toContainText("/offres/[plan]");

  await panel.getByLabel("Titre (SEO)").fill("Tarifs de Deep Atelier");
  await panel
    .getByLabel("Description (SEO)")
    .fill("Des tarifs simples, sans surprise.");
  await panel.getByLabel("Layout partagé").selectOption({ label: "site" });
  // Le layout entoure la page : son en-tête apparaît dans le canvas.
  await expect(frame(page, 390).locator("header")).toBeVisible();

  await expect(page.locator('[data-save="saved"]')).toBeVisible();
  await page.reload();
  await expect(page.getByRole("heading", { name: "Landing" })).toBeVisible();
  await page.getByLabel("Page", { exact: true }).selectOption({
    label: "Nos tarifs",
  });
  await openLeftPanel(page, "Pages");
  await expect(panel.getByLabel("Route")).toHaveValue("/offres/[plan]");
  await expect(panel.getByLabel("Titre (SEO)")).toHaveValue(
    "Tarifs de Deep Atelier",
  );
  await expect(panel.getByLabel("Layout partagé")).toHaveValue(/^l_/);
  await expectNoHorizontalScroll(page);
});

test("tokens: a color re-themes the canvas, a font comes from the catalogue", async ({
  page,
}) => {
  await openLanding(page);
  await openLeftPanel(page, "Tokens");
  const panel = page.getByTestId("tokens-panel");
  const cta = frame(page, 390).getByRole("link", {
    name: "Commencer gratuitement",
  });
  await expect(cta).not.toHaveCSS("background-color", "rgb(255, 0, 0)");

  const primary = panel.locator('[data-token="primary"]');
  await primary
    .getByRole("textbox", { name: "primary, clair" })
    .fill("#ff0000");
  await page.keyboard.press("Enter");
  await expect(cta).toHaveCSS("background-color", "rgb(255, 0, 0)");
  // Une valeur qui n'est pas une couleur est abandonnée.
  await primary.getByRole("textbox", { name: "primary, clair" }).fill("rouge");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Tab");
  await expect(
    primary.getByRole("textbox", { name: "primary, clair" }),
  ).toHaveValue("#ff0000");

  // Un token utilisé ne se supprime pas.
  await primary.getByRole("button", { name: "Supprimer primary" }).click();
  await expect(page.getByRole("alert")).toContainText("Encore utilisé");
  await expect(primary).toBeVisible();

  // Police du catalogue : la feuille Google Fonts est chargée avec des graisses qui existent.
  const sans = panel.locator('[data-token="sans"]');
  await sans.getByLabel("Famille de sans").selectOption("Inter");
  await expect(sans.getByLabel("400")).toBeChecked();
  await expect(sans.getByLabel("700")).toBeChecked();
  await sans.getByLabel("500").check();
  await expect(
    frame(page, 390).locator(
      'link[rel="stylesheet"][href*="family=Inter:wght@400;500;700"]',
    ),
  ).toHaveCount(1);

  // Nouveau token de couleur.
  await panel.getByLabel("Nouvelle couleur").fill("brand");
  await panel.getByRole("button", { name: "Ajouter" }).first().click();
  await expect(panel.locator('[data-token="brand"]')).toBeVisible();
  await expect(page.locator('[data-save="saved"]')).toBeVisible();
  await expectNoHorizontalScroll(page);
});
