"use server";

import { cookies } from "next/headers";

import { isLocale, localeCookie } from "@/i18n/config";

/** Garde la langue choisie (un an). */
export async function setLocale(locale: string): Promise<void> {
  if (!isLocale(locale)) return;
  (await cookies()).set(localeCookie, locale, {
    path: "/",
    maxAge: 60 * 60 * 24 * 365,
    sameSite: "lax",
  });
}
