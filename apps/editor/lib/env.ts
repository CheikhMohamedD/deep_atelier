// Variables publiques de l'éditeur (préfixe NEXT_PUBLIC_, injectées au build). Lues à la
// demande : une page qui n'en a pas besoin se construit sans elles.

export type PublicEnv = {
  supabaseUrl: string;
  supabaseKey: string;
  apiUrl: string;
};

function required(name: string, value: string | undefined): string {
  if (!value) {
    throw new Error(
      `Variable d'environnement manquante : ${name} (voir apps/editor/.env.example)`,
    );
  }
  return value;
}

export function publicEnv(): PublicEnv {
  return {
    supabaseUrl: required(
      "NEXT_PUBLIC_SUPABASE_URL",
      process.env.NEXT_PUBLIC_SUPABASE_URL,
    ),
    supabaseKey: required(
      "NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY",
      process.env.NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY,
    ),
    apiUrl: required(
      "NEXT_PUBLIC_API_URL",
      process.env.NEXT_PUBLIC_API_URL,
    ).replace(/\/$/, ""),
  };
}
