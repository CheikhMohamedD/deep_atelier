// Document de démonstration utilisé pour peupler les projets des tests (cargo xtask demo).

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";

import { LANDING_FIXTURE } from "./helpers";

export default function globalSetup() {
  if (existsSync(LANDING_FIXTURE)) return;
  try {
    execFileSync("cargo", ["xtask", "demo", "landing", LANDING_FIXTURE], {
      stdio: "inherit",
    });
  } catch {
    throw new Error(
      `Document de démonstration absent : lancer \`cargo xtask demo landing ${LANDING_FIXTURE}\``,
    );
  }
}
