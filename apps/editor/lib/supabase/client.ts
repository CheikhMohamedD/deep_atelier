import { createBrowserClient } from "@supabase/ssr";
import type { SupabaseClient } from "@supabase/supabase-js";

import { publicEnv } from "@/lib/env";

let client: SupabaseClient | undefined;

/** Client Supabase du navigateur (session dans les cookies, partagée avec le serveur). */
export function supabaseBrowser(): SupabaseClient {
  if (!client) {
    const env = publicEnv();
    client = createBrowserClient(env.supabaseUrl, env.supabaseKey);
  }
  return client;
}
