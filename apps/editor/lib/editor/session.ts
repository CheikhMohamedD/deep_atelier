// Session d'édition d'un projet : charge le document par l'API, l'ouvre dans le moteur
// WebAssembly, applique les commandes, l'undo et le redo, rafraîchit le canvas et enregistre
// automatiquement.

import { Engine, loadEngine } from "@deep-atelier/engine";
import type {
  ChangeSet,
  Command,
  Document,
  Transaction,
} from "@deep-atelier/ir-types";

import { ApiError, api, errorCode } from "@/lib/api";

import { Autosave } from "./autosave";
import { useEditor, type PageInfo } from "./store";

/** Module WebAssembly servi par l'éditeur (scripts/prepare-assets.mjs). */
const WASM_URL = "/engine/engine_bg.wasm";

function pagesOf(doc: Document): PageInfo[] {
  return doc.pages.map((page) => ({ id: page.id, name: page.name }));
}

/** Transaction d'un geste de l'utilisateur. */
export function userTransaction(
  label: string,
  commands: Command[],
): Transaction {
  return {
    label,
    origin: { kind: "user" },
    scope: { content_only: false },
    commands,
  };
}

export class EditorSession {
  readonly projectId: string;
  #engine: Engine | null = null;
  #autosave: Autosave | null = null;
  /** Les réponses d'une ouverture remplacée ou d'une session fermée sont ignorées. */
  #generation = 0;

  constructor(projectId: string) {
    this.projectId = projectId;
  }

  /** Vrai si des changements ne sont pas encore enregistrés. */
  get unsaved(): boolean {
    return this.#autosave?.unsaved ?? false;
  }

  async open(): Promise<void> {
    const generation = ++this.#generation;
    this.#close();
    useEditor.getState().reset();
    try {
      const [project, document] = await Promise.all([
        api.getProject(this.projectId),
        api.getDocument(this.projectId),
        loadEngine(WASM_URL),
      ]);
      if (generation !== this.#generation) return;
      const engine = Engine.open(document.document);
      this.#engine = engine;
      this.#autosave = new Autosave(
        async (base) =>
          (
            await api.saveDocument(this.projectId, {
              base_version: base,
              document: engine.document(),
            })
          ).version,
        document.version,
        {
          onStatus: (status) => useEditor.getState().setSave(status),
          isConflict: (error) =>
            error instanceof ApiError && error.code === "VERSION_CONFLICT",
        },
      );
      const doc = engine.document();
      const pageId = doc.pages[0]?.id ?? null;
      useEditor.getState().ready({
        projectName: project.project.name,
        pages: pagesOf(doc),
        pageId,
        canvas: pageId ? engine.canvasPage(pageId) : null,
      });
    } catch (error) {
      if (generation === this.#generation)
        useEditor.getState().failed(errorCode(error));
    }
  }

  /** Applique une transaction ; une commande refusée laisse le document intact. */
  run(transaction: Transaction): boolean {
    const engine = this.#engine;
    if (!engine) return false;
    const result = engine.apply(transaction);
    if (!result.ok) {
      useEditor.getState().setNotice(result.error.code);
      return false;
    }
    useEditor.getState().setNotice(null);
    this.#changed(result.applied.changes);
    return true;
  }

  undo(): void {
    const result = this.#engine?.undo();
    if (result?.ok && result.changes) this.#changed(result.changes);
  }

  redo(): void {
    const result = this.#engine?.redo();
    if (result?.ok && result.changes) this.#changed(result.changes);
  }

  /** Supprime les nœuds sélectionnés (jamais une racine de page ou de layout). */
  deleteSelection(): void {
    const engine = this.#engine;
    const { selection } = useEditor.getState();
    if (!engine || selection.length === 0) return;
    const doc = engine.document();
    const roots = new Set([
      ...doc.pages.map((page) => page.root),
      ...doc.layouts.map((layout) => layout.root),
    ]);
    const nodes = [...new Set(selection.map((target) => target.node))].filter(
      (node) => !roots.has(node),
    );
    if (nodes.length === 0) return;
    if (this.run(userTransaction("Delete", [{ op: "delete_nodes", nodes }]))) {
      useEditor.getState().clearSelection();
    }
  }

  setPage(pageId: string): void {
    const engine = this.#engine;
    if (!engine) return;
    const canvas = engine.canvasPage(pageId);
    if (canvas) useEditor.getState().setPage(pageId, canvas);
  }

  /** Enregistre tout de suite ce qui attend. */
  flush(): Promise<void> {
    return this.#autosave?.flush() ?? Promise.resolve();
  }

  /** Ferme la session : enregistre ce qui attend, puis libère le moteur. */
  dispose(): void {
    this.#generation += 1;
    const autosave = this.#autosave;
    const engine = this.#engine;
    this.#autosave = null;
    this.#engine = null;
    void (autosave?.flush() ?? Promise.resolve()).finally(() => {
      autosave?.dispose();
      engine?.dispose();
    });
  }

  #changed(changes: ChangeSet): void {
    const engine = this.#engine;
    if (!engine) return;
    const state = useEditor.getState();
    let pageId = state.pageId;
    const pages =
      changes.pages.length > 0 ? pagesOf(engine.document()) : undefined;
    if (pages && !pages.some((page) => page.id === pageId)) {
      pageId = pages[0]?.id ?? null;
    }
    const canvas = pageId ? engine.canvasPage(pageId) : null;
    if (pageId !== state.pageId && pageId) {
      state.setPage(pageId, canvas);
    }
    state.setCanvas(
      canvas,
      { canUndo: engine.canUndo, canRedo: engine.canRedo },
      pages,
    );
    this.#autosave?.changed();
  }

  #close(): void {
    this.#autosave?.dispose();
    this.#autosave = null;
    this.#engine?.dispose();
    this.#engine = null;
  }
}
