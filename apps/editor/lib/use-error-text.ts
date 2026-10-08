"use client";

import { useTranslations } from "next-intl";
import { useCallback } from "react";

/** Message traduit d'un code d'erreur de l'API (générique pour un code inconnu). */
export function useErrorText(): (code: string) => string {
  const t = useTranslations("errors");
  return useCallback(
    (code: string) =>
      t.has(code as "INTERNAL") ? t(code as "INTERNAL") : t("INTERNAL"),
    [t],
  );
}
