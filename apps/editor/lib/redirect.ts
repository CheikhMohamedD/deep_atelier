/** Chemin interne où revenir après la connexion (jamais une autre origine). */
export function safeNext(value: string | null | undefined): string {
  return value &&
    value.startsWith("/") &&
    !value.startsWith("//") &&
    !value.startsWith("/\\")
    ? value
    : "/projects";
}
