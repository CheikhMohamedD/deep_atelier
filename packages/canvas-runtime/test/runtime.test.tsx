import { readFrameMessage, wrap } from "@deep-atelier/canvas-protocol";
import type { CanvasPage } from "@deep-atelier/ir-types";
import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Frame, applyTheme, keyOf, measure, toggle } from "../src/frame";
import { CanvasRoot, iconComponent } from "../src/render";

const page: CanvasPage = {
  page: "p_0000000001",
  lang: "fr",
  css: ":root {\n  --primary: #171717;\n}\n",
  font_stylesheets: [
    "https://fonts.googleapis.com/css2?family=Inter&display=swap",
  ],
  truncated: false,
  nodes: [
    {
      type: "element",
      key: "n_header",
      node: "n_header",
      tag: "header",
      class: "flex items-center",
      attrs: {},
      children: [
        {
          type: "element",
          key: "n_burger",
          node: "n_burger",
          tag: "button",
          class: "p-2 md:hidden",
          attrs: { type: "button", "aria-label": "Menu" },
          toggles: "n_nav",
          children: [
            {
              type: "element",
              key: "n_icon",
              node: "n_icon",
              tag: "svg",
              class: "w-6 h-6",
              attrs: { "aria-hidden": "true" },
              icon: "menu",
              children: [],
            },
          ],
        },
        {
          type: "element",
          key: "n_nav",
          node: "n_nav",
          tag: "nav",
          class: "hidden data-[open=true]:flex",
          attrs: {},
          children: [
            {
              type: "element",
              key: "n_card/n_link",
              node: "n_link",
              instance: "n_card",
              tag: "a",
              class: "text-sm",
              attrs: { href: "#", "data-page": "p_0000000002" },
              children: [
                { type: "text", text: "Tarifs " },
                {
                  type: "element",
                  tag: "strong",
                  class: "",
                  attrs: {},
                  children: [{ type: "text", text: "pro" }],
                },
              ],
            },
          ],
        },
      ],
    },
    {
      type: "element",
      key: "n_input",
      node: "n_input",
      tag: "input",
      class: "border",
      attrs: { type: "email", name: "email", required: "" },
      children: [],
    },
    {
      type: "raw",
      key: "n_raw",
      node: "n_raw",
      code: '<time dateTime="2026-10-08">8 octobre</time>',
      imports: [],
      client: false,
    },
  ],
};

afterEach(() => {
  document.body.innerHTML = "";
  document.head.innerHTML = "";
});

describe("CanvasRoot", () => {
  it("renders the export's elements with their keys and classes", () => {
    const onCommit = vi.fn();
    const { container } = render(
      <CanvasRoot page={page} onCommit={onCommit} />,
    );
    expect(onCommit).toHaveBeenCalled();
    const header = container.querySelector('[data-atl-key="n_header"]');
    expect(header?.tagName).toBe("HEADER");
    expect(header?.className).toBe("flex items-center");
    const burger = container.querySelector('[data-atl-key="n_burger"]');
    expect(burger?.getAttribute("aria-label")).toBe("Menu");
    expect(burger?.getAttribute("data-atl-toggles")).toBe("n_nav");
    const icon = container.querySelector('[data-atl-key="n_icon"]');
    expect(icon?.tagName.toLowerCase()).toBe("svg");
    expect(icon?.getAttribute("class")).toContain("w-6 h-6");
    expect(icon?.getAttribute("class")).toContain("lucide-menu");
    const link = container.querySelector('[data-atl-key="n_card/n_link"]');
    expect(link?.textContent).toBe("Tarifs pro");
    expect(link?.querySelector("strong")?.hasAttribute("data-atl-key")).toBe(
      false,
    );
    const input = container.querySelector("input");
    expect(input?.required).toBe(true);
    expect(input?.type).toBe("email");
    const raw = container.querySelector('[data-atl-key="n_raw"]');
    expect(raw?.textContent).toContain("8 octobre");
  });

  it("maps IR icon names to lucide components", () => {
    expect(iconComponent("menu")).toBeDefined();
    expect(iconComponent("arrow-right")).toBeDefined();
    expect(iconComponent("no-such-icon")).toBeUndefined();
  });
});

describe("frame helpers", () => {
  it("finds keys, measures elements and toggles targets", () => {
    const { container } = render(<CanvasRoot page={page} />);
    const strong = container.querySelector("strong");
    expect(keyOf(strong)).toBe("n_card/n_link");
    expect(keyOf(null)).toBeNull();
    const rects = measure(document);
    expect(Object.keys(rects).sort()).toEqual(
      [
        "n_burger",
        "n_card/n_link",
        "n_header",
        "n_icon",
        "n_input",
        "n_nav",
        "n_raw",
      ].sort(),
    );
    const burger = container.querySelector('[data-atl-key="n_burger"]')!;
    const nav = container.querySelector('[data-atl-key="n_nav"]')!;
    toggle(document, burger);
    expect(nav.getAttribute("data-open")).toBe("true");
    expect(burger.getAttribute("aria-expanded")).toBe("true");
    toggle(document, burger);
    expect(nav.getAttribute("data-open")).toBe("false");
  });

  it("applies the theme, fonts and language once", () => {
    const style = document.createElement("style");
    style.id = "atl-theme";
    style.setAttribute("type", "text/tailwindcss");
    document.head.append(style);
    applyTheme(document, page);
    applyTheme(document, page);
    expect(style.textContent).toBe(page.css);
    expect(document.documentElement.lang).toBe("fr");
    expect(document.querySelectorAll("link[data-atl-font]")).toHaveLength(1);
  });
});

describe("Frame", () => {
  it("renders on request, answers hit tests and plays links in preview", async () => {
    const posted: unknown[] = [];
    vi.spyOn(window, "postMessage").mockImplementation((message: unknown) => {
      posted.push(message);
    });
    const rendered: CanvasPage[] = [];
    const frame = new Frame(window, (next) => {
      rendered.push(next);
      render(<CanvasRoot page={next} />);
    });
    frame.start();
    const messages = () => posted.map(readFrameMessage);
    expect(messages()).toContainEqual({ type: "ready" });

    const send = (data: unknown) =>
      window.dispatchEvent(
        new MessageEvent("message", { data, source: window }),
      );
    send(wrap({ type: "render", page }));
    expect(rendered).toHaveLength(1);
    // Un message d'une autre fenêtre est ignoré.
    window.dispatchEvent(
      new MessageEvent("message", { data: wrap({ type: "render", page }) }),
    );
    expect(rendered).toHaveLength(1);

    const link = document.querySelector('[data-atl-key="n_card/n_link"]')!;
    document.elementFromPoint = () => link;
    send(wrap({ type: "hit_test", id: 7, x: 10, y: 20 }));
    expect(messages()).toContainEqual({
      type: "hit",
      id: 7,
      key: "n_card/n_link",
    });

    // En mode design, un lien ne fait rien ; en Preview, il ouvre sa page.
    (link as HTMLElement).click();
    expect(messages()).not.toContainEqual({
      type: "navigate",
      page: "p_0000000002",
    });
    send(wrap({ type: "mode", mode: "preview" }));
    expect(frame.mode).toBe("preview");
    (link as HTMLElement).click();
    expect(messages()).toContainEqual({
      type: "navigate",
      page: "p_0000000002",
    });
    const burger = document.querySelector(
      '[data-atl-key="n_burger"]',
    ) as HTMLElement;
    burger.click();
    expect(
      document
        .querySelector('[data-atl-key="n_nav"]')
        ?.getAttribute("data-open"),
    ).toBe("true");
  });
});
