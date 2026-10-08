// Client de l'API Rust (crates/api), typé par le contrat généré (@deep-atelier/ir-types).

import type {
  CreatedProject,
  DocumentState,
  ErrorBody,
  NewProject,
  ProjectList,
  ProjectResponse,
  Saved,
  SaveDocument,
} from "@deep-atelier/ir-types";

import { publicEnv } from "@/lib/env";
import { supabaseBrowser } from "@/lib/supabase/client";

/** Erreur d'un appel : code de l'API, ou `NETWORK` si le serveur ne répond pas. */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  /** `VERSION_CONFLICT` : version courante du document. */
  readonly currentVersion: number | undefined;

  constructor(
    status: number,
    code: string,
    message: string,
    currentVersion?: number,
  ) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.currentVersion = currentVersion;
  }
}

async function accessToken(): Promise<string> {
  const { data } = await supabaseBrowser().auth.getSession();
  const token = data.session?.access_token;
  if (!token) throw new ApiError(401, "UNAUTHORIZED", "no session");
  return token;
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${await accessToken()}`,
  };
  const init: RequestInit = { method, headers };
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  let response: Response;
  try {
    response = await fetch(`${publicEnv().apiUrl}${path}`, init);
  } catch (error) {
    throw new ApiError(
      0,
      "NETWORK",
      error instanceof Error ? error.message : String(error),
    );
  }
  if (response.status === 204) return undefined as T;
  const json: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const detail = (json as ErrorBody | null)?.error;
    throw new ApiError(
      response.status,
      detail?.code ?? "INTERNAL",
      detail?.message ?? response.statusText,
      detail?.current_version,
    );
  }
  return json as T;
}

export const api = {
  listProjects: () => request<ProjectList>("GET", "/projects"),
  createProject: (body: NewProject) =>
    request<CreatedProject>("POST", "/projects", body),
  deleteProject: (id: string) => request<void>("DELETE", `/projects/${id}`),
  getProject: (id: string) =>
    request<ProjectResponse>("GET", `/projects/${id}`),
  getDocument: (id: string) =>
    request<DocumentState>("GET", `/projects/${id}/document`),
  saveDocument: (id: string, body: SaveDocument) =>
    request<Saved>("PUT", `/projects/${id}/document`, body),
};

/** Code d'erreur à traduire (`errors.<code>`), connu ou générique. */
export function errorCode(error: unknown): string {
  return error instanceof ApiError ? error.code : "INTERNAL";
}
