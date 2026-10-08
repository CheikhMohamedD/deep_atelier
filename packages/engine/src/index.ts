// Moteur de l'éditeur : session de l'IR (commandes, undo/redo, gestes), rendu des pages pour le
// canvas et validation, exécutés en WebAssembly (crates/engine-wasm). Les échanges avec le
// module se font en JSON ; ce wrapper les type avec @deep-atelier/ir-types.

import type {
  Applied,
  CanvasPage,
  ChangeSet,
  Document,
  Issue,
  Transaction,
} from "@deep-atelier/ir-types";

import init, { Engine as WasmEngine } from "../pkg/engine.js";

/** Erreur d'une commande ou du moteur, avec un code stable (traduit par l'interface). */
export type EngineError = { code: string; message: string };

export type ApplyResult =
  { ok: true; applied: Applied } | { ok: false; error: EngineError };

/** Résultat d'un undo ou d'un redo ; `changes` vaut `null` s'il n'y avait rien à faire. */
export type HistoryResult =
  { ok: true; changes: ChangeSet | null } | { ok: false; error: EngineError };

/** Source du module : URL du fichier `.wasm`, ou ses octets (Node, tests). */
export type WasmSource = string | URL | BufferSource | WebAssembly.Module;

let loading: Promise<unknown> | undefined;

/** Charge le module WebAssembly, une seule fois. */
export async function loadEngine(source?: WasmSource): Promise<void> {
  loading ??= init(
    source === undefined ? undefined : { module_or_path: source },
  );
  await loading;
}

/** Graine du générateur d'identifiants du moteur. */
function seed(): Uint8Array {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  return bytes;
}

export class Engine {
  readonly #wasm: WasmEngine;

  private constructor(wasm: WasmEngine) {
    this.#wasm = wasm;
  }

  /** Ouvre un document de l'IR (le module doit être chargé : `loadEngine`). */
  static open(document: Document | string): Engine {
    const json =
      typeof document === "string" ? document : JSON.stringify(document);
    return new Engine(new WasmEngine(json, seed()));
  }

  /** Document neuf, sans l'API : une page d'accueil vide. */
  static blank(name: string): Engine {
    return new Engine(WasmEngine.blank(name, seed()));
  }

  document(): Document {
    return JSON.parse(this.#wasm.document()) as Document;
  }

  /** Document sérialisé, tel que l'API l'enregistre. */
  documentJson(): string {
    return this.#wasm.document();
  }

  /** Applique une transaction de façon atomique : en cas d'erreur, rien ne change. */
  apply(transaction: Transaction): ApplyResult {
    return JSON.parse(
      this.#wasm.apply(JSON.stringify(transaction)),
    ) as ApplyResult;
  }

  undo(): HistoryResult {
    return JSON.parse(this.#wasm.undo()) as HistoryResult;
  }

  redo(): HistoryResult {
    return JSON.parse(this.#wasm.redo()) as HistoryResult;
  }

  get canUndo(): boolean {
    return this.#wasm.canUndo();
  }

  get canRedo(): boolean {
    return this.#wasm.canRedo();
  }

  /** Geste continu (curseur, glisser) : ses transactions forment une seule entrée d'undo. */
  beginGesture(label: string): void {
    this.#wasm.beginGesture(label);
  }

  endGesture(): void {
    this.#wasm.endGesture();
  }

  validate(): Issue[] {
    return JSON.parse(this.#wasm.validate()) as Issue[];
  }

  /** Rendu d'une page pour le canvas ; `null` si la page n'existe pas. */
  canvasPage(page: string): CanvasPage | null {
    return JSON.parse(this.#wasm.canvasPage(page)) as CanvasPage | null;
  }

  /** Libère la mémoire WebAssembly de la session. */
  dispose(): void {
    this.#wasm.free();
  }
}
