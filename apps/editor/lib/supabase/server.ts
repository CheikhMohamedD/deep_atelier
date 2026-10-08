import { createServerClient } from "@supabase/ssr";
import type { SupabaseClient } from "@supabase/supabase-js";
import { cookies } from "next/headers";

import { publicEnv } from "@/lib/env";

/** Client Supabase d'un composant serveur ou d'une route (cookies de la requête). */
export async function supabaseServer(): Promise<SupabaseClient> {
  const store = await cookies();
  const env = publicEnv();
  return createServerClient(env.supabaseUrl, env.supabaseKey, {
    cookies: {
      getAll: () => store.getAll(),
      setAll: (cookiesToSet) => {
        try {
          for (const { name, value, options } of cookiesToSet) {
            store.set(name, value, options);
          }
        } catch {
          // Composant serveur : les cookies sont rafraîchis par le middleware.
        }
      },
    },
  });
}
