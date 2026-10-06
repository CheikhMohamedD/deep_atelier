# Atelier — consignes pour les agents

## À lire avant toute tâche

1. `docs/SPEC.md` : vision, prompt maître (principes non négociables, stack imposée, méthode, critères de succès).
2. `docs/adr/` : décisions d'architecture. `0001-architecture-phase1.md` fait référence pour la Phase 1
   (schéma de l'IR, commandes, outils LLM, découpage des étapes, questions ouvertes).

## Méthode

- Une étape = une branche = une PR, dans l'ordre de l'ADR 0001 § 11.
- Avant de coder une étape, poser au propriétaire les questions qui la concernent (ADR 0001 § 12),
  **une par une**, avec l'outil de question (options + recommandation), et attendre chaque réponse.
  Consigner la décision dans l'ADR avant d'implémenter.
- Code complet à chaque étape : pas de pseudo-code, pas de TODO. Tests inclus, plus une courte note
  sur les choix faits et les limites connues dans la description de la PR.
- Spécification ambiguë → poser la question au propriétaire, ne pas inventer. Une décision nouvelle
  ou un écart à la spec → nouvel ADR ou mise à jour de l'ADR concerné.
- Échanges avec le propriétaire en français. Identifiants de code en anglais ; commentaires en français.

## Règles qui ne se négocient pas

- Toute mutation de l'IR passe par une `Command` ; jamais de modification directe.
- Aucun concept DOM/CSS dans le cœur de l'IR (les échappatoires web vivent dans `platform_overrides.web`).
- Mobile-first : aucune fonctionnalité n'est terminée sans test aux largeurs 390, 768 et 1280 px.
- Le code exporté ne dépend jamais d'Atelier au runtime.
- Les types TS et les schémas d'outils sont générés (`cargo xtask codegen`) : ne jamais les éditer à la main.

## Dépôt public

- Avant tout commit, régler l'identité git locale :
  `git config user.name "Cheikh Mohamed"` et
  `git config user.email "85410449+CheikhMohamedD@users.noreply.github.com"`. Jamais d'email professionnel.
- Aucun secret, aucune clé, aucun identifiant de compte (organisation Supabase, ids de projet) dans
  le dépôt. Les secrets vont dans l'environnement cloud (ADR 0001 § 13).
