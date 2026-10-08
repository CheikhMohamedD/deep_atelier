// Sauvegarde automatique : différée après chaque changement, une seule à la fois, arrêtée sur
// un conflit de version, relancée après une erreur réseau ou serveur.

export type SaveStatus = "saved" | "dirty" | "saving" | "error" | "conflict";

/** Enregistre le document courant à partir de la version `base` ; renvoie la nouvelle version. */
export type Saver = (base: number) => Promise<number>;

export type AutosaveOptions = {
  onStatus: (status: SaveStatus) => void;
  /** Erreur de conflit de version : la sauvegarde s'arrête, l'utilisateur recharge. */
  isConflict: (error: unknown) => boolean;
  debounceMs?: number;
  retryMs?: number;
};

export class Autosave {
  #version: number;
  #status: SaveStatus = "saved";
  #pending = false;
  #saving = false;
  #timer: ReturnType<typeof setTimeout> | undefined;
  readonly #save: Saver;
  readonly #options: Required<AutosaveOptions>;

  constructor(save: Saver, version: number, options: AutosaveOptions) {
    this.#save = save;
    this.#version = version;
    this.#options = { debounceMs: 1000, retryMs: 5000, ...options };
  }

  get version(): number {
    return this.#version;
  }

  get status(): SaveStatus {
    return this.#status;
  }

  /** Vrai si des changements ne sont pas encore enregistrés. */
  get unsaved(): boolean {
    return this.#pending || this.#saving;
  }

  /** Le document a changé. */
  changed(): void {
    if (this.#status === "conflict") return;
    this.#pending = true;
    if (!this.#saving) this.#set("dirty");
    this.#schedule(this.#options.debounceMs);
  }

  /** Enregistre tout de suite ce qui attend. */
  async flush(): Promise<void> {
    this.#clear();
    if (this.#saving || !this.#pending || this.#status === "conflict") return;
    this.#saving = true;
    this.#pending = false;
    this.#set("saving");
    try {
      this.#version = await this.#save(this.#version);
      this.#saving = false;
      if (this.#pending) {
        this.#set("dirty");
        this.#schedule(this.#options.debounceMs);
      } else {
        this.#set("saved");
      }
    } catch (error) {
      this.#saving = false;
      this.#pending = true;
      if (this.#options.isConflict(error)) {
        this.#set("conflict");
      } else {
        this.#set("error");
        this.#schedule(this.#options.retryMs);
      }
    }
  }

  dispose(): void {
    this.#clear();
  }

  #schedule(delay: number): void {
    this.#clear();
    this.#timer = setTimeout(() => void this.flush(), delay);
  }

  #clear(): void {
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.#timer = undefined;
  }

  #set(status: SaveStatus): void {
    if (status === this.#status) return;
    this.#status = status;
    this.#options.onStatus(status);
  }
}
