// Session d'édition d'un projet : charge le document par l'API, l'ouvre dans le moteur
// WebAssembly, applique les commandes, l'undo et le redo, rafraîchit le canvas et enregistre
// automatiquement.

import { Engine, loadEngine } from "@deep-atelier/engine";
import type {
  Command,
  NodeInspection,
  Transaction,
} from "@deep-atelier/ir-types";

import { ApiError, api, errorCode } from "@/lib/api";

import { Autosave } from "./autosave";
import { useEditor } from "./store";
import { isLocked, rootNodes } from "./tree";

/** Module WebAssembly servi par l'éditeur (scripts/prepare-assets.mjs). */
const WASM_URL = "/engine/engine_bg.wasm";

/**
 * Transaction d'un geste de l'utilisateur. En mode Contenu, le périmètre `content_only` de l'IR
 * refuse tout ce qui n'est pas un texte, une image ou un lien.
 */
export function userTransaction(
  label: string,
  commands: Command[],
  contentOnly = false,
): Transaction {
  return {
    label,
    origin: { kind: "user" },
    scope: { content_only: contentOnly },
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
        doc,
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
    this.#changed();
    return true;
  }

  /** Applique des commandes dans le périmètre du mode courant. */
  edit(label: string, commands: Command[]): boolean {
    if (commands.length === 0) return false;
    const contentOnly = useEditor.getState().mode === "content";
    return this.run(userTransaction(label, commands, contentOnly));
  }

  /**
   * Crée une page et l'affiche. Passe par `run` pour lire l'identifiant de la page créée.
   */
  createPage(command: Extract<Command, { op: "create_page" }>): boolean {
    const engine = this.#engine;
    if (!engine) return false;
    const result = engine.apply(userTransaction("Create page", [command]));
    if (!result.ok) {
      useEditor.getState().setNotice(result.error.code);
      return false;
    }
    useEditor.getState().setNotice(null);
    this.#changed();
    const created = result.applied.pages[0];
    if (created) this.setPage(created);
    return true;
  }

  /** Geste continu (curseur, sélecteur de couleur) : une seule entrée d'undo. */
  beginGesture(label: string): void {
    this.#engine?.beginGesture(label);
  }

  endGesture(): void {
    this.#engine?.endGesture();
  }

  /** Classes produites, propriétés permises et problèmes d'un nœud. */
  inspect(node: string): NodeInspection | null {
    return this.#engine?.inspect(node) ?? null;
  }

  undo(): void {
    const result = this.#engine?.undo();
    if (result?.ok && result.changes) this.#changed();
  }

  redo(): void {
    const result = this.#engine?.redo();
    if (result?.ok && result.changes) this.#changed();
  }

  /** Supprime les nœuds sélectionnés (jamais une racine, ni un calque verrouillé). */
  deleteSelection(): void {
    const { selection, doc, mode } = useEditor.getState();
    if (!doc || mode !== "design" || selection.length === 0) return;
    const roots = rootNodes(doc);
    const nodes = [...new Set(selection.map((target) => target.node))].filter(
      (node) => !roots.has(node) && !isLocked(doc, node),
    );
    if (nodes.length === 0) return;
    if (this.edit("Delete", [{ op: "delete_nodes", nodes }])) {
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

  /** Relit le document dans le moteur, puis rafraîchit le canvas et lance la sauvegarde. */
  #changed(): void {
    const engine = this.#engine;
    if (!engine) return;
    const state = useEditor.getState();
    const doc = engine.document();
    let pageId = state.pageId;
    if (!doc.pages.some((page) => page.id === pageId)) {
      pageId = doc.pages[0]?.id ?? null;
    }
    const canvas = pageId ? engine.canvasPage(pageId) : null;
    if (pageId !== state.pageId) state.setPage(pageId, canvas);
    state.setDocument(doc, canvas, {
      canUndo: engine.canUndo,
      canRedo: engine.canRedo,
    });
    this.#autosave?.changed();
  }

  #close(): void {
    this.#autosave?.dispose();
    this.#autosave = null;
    this.#engine?.dispose();
    this.#engine = null;
  }
}
