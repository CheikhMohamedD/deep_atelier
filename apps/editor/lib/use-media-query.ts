"use client";

import { useSyncExternalStore } from "react";

/** Vrai quand la requête média correspond (faux au rendu serveur). */
export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const list = window.matchMedia(query);
      list.addEventListener("change", onChange);
      return () => list.removeEventListener("change", onChange);
    },
    () => window.matchMedia(query).matches,
    () => false,
  );
}

/** Téléphone : sous 768 px, l'éditeur n'offre que le mode Contenu et l'aperçu (spec, § Modes). */
export const PHONE_QUERY = "(max-width: 767px)";
