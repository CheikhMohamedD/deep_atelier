import { publicEnv } from "@/lib/env";

/**
 * Fournisseurs de connexion actifs, d'après les réglages publics de Supabase Auth : un bouton
 * de fournisseur désactivé mènerait à une erreur 400 (« provider is not enabled »).
 */
export async function enabledProviders(): Promise<{ github: boolean }> {
  try {
    const env = publicEnv();
    const response = await fetch(`${env.supabaseUrl}/auth/v1/settings`, {
      headers: { apikey: env.supabaseKey },
      cache: "no-store",
    });
    if (!response.ok) return { github: false };
    const settings = (await response.json()) as {
      external?: Record<string, unknown>;
    };
    return { github: settings.external?.github === true };
  } catch {
    return { github: false };
  }
}
