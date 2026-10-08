// Point d'entrée de l'iframe du canvas.

import "@tailwindcss/browser";

import { createRoot } from "react-dom/client";

import { Frame } from "./frame";
import { CanvasRoot } from "./render";

const container = document.getElementById("atl-root");
if (container) {
  const root = createRoot(container);
  const frame: Frame = new Frame(window, (page) => {
    root.render(<CanvasRoot page={page} onCommit={() => frame.schedule()} />);
  });
  frame.start();
}
