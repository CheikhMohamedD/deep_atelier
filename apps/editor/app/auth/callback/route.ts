// Retour de GitHub ou du lien magique : le code PKCE est échangé contre une session (cookies).

import { NextResponse, type NextRequest } from "next/server";

import { safeNext } from "@/lib/redirect";
import { supabaseServer } from "@/lib/supabase/server";

export async function GET(request: NextRequest) {
  const { searchParams, origin } = request.nextUrl;
  const code = searchParams.get("code");
  const next = safeNext(searchParams.get("next"));
  if (code) {
    const supabase = await supabaseServer();
    const { error } = await supabase.auth.exchangeCodeForSession(code);
    if (!error) return NextResponse.redirect(new URL(next, origin));
  }
  return NextResponse.redirect(new URL("/login?error=callback", origin));
}
