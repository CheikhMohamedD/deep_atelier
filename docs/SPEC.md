# Website Builder dev-first — Prompt & Spécifications

Oct 6, 2026 · @Cheikh Mohamed

## Vision & positionnement

Deep Atelier (nom de code) est un website builder où le canvas visuel est l'interface principale, mais où chaque action produit du vrai code lisible, versionné et exportable. Cible v1 : sites et apps web en React/Next.js, responsive mobile-first. Cible v2 : apps mobiles natives depuis le même projet.

**La promesse en une phrase :** la vitesse d'un outil visuel, la liberté d'un IDE, sans lock-in.

| Catégorie | Exemples | Leur limite | Notre réponse |
| --- | --- | --- | --- |
| IDE classiques | VS Code, WebStorm | Texte d'abord, la UI se voit seulement au runtime | Canvas d'abord, le code est une vue synchronisée |
| No-code | Wix, Webflow, Framer | Code propriétaire ou peu exploitable, plafond vite atteint | Code React/TS propre, export Git, composants custom |
| Générateurs IA | v0, Bolt, Lovable | Prompt → code opaque, retouche visuelle faible | IA intégrée au canvas, chaque changement reste éditable visuellement |

**Les 5 différenciateurs :**

1. **Bidirectionnel visuel ⇄ code** : une modification sur le canvas réécrit le code ; une modification du code met à jour le canvas, en temps réel.
2. **Prompt comme geste de création** : on construit en décrivant autant qu'en déplaçant ; l'IA écrit dans l'IR, donc chaque génération reste éditable au canvas.
3. **Code propriété du dev** : sortie TypeScript + Tailwind idiomatique, push sur GitHub, aucun runtime propriétaire.
4. **Responsive natif** : chaque propriété de style est par breakpoint, la vue mobile est la vue par défaut.
5. **Moteur indépendant de la plateforme** : le projet est stocké dans une représentation intermédiaire (IR), compilée vers le web aujourd'hui et vers React Native demain.

## Prompt maître

À coller tel quel dans Claude Code, Cursor ou un agent équivalent, avec ce document joint comme référence. Il cadre l'agent sur la phase 1 (MVP) et lui interdit de sauter les fondations qui rendent la v2 mobile possible.

```markdown
RÔLE
Tu es un architecte logiciel senior et un ingénieur expert en Rust, WebAssembly, React/TypeScript, compilateurs AST, éditeurs visuels (type Figma/Webflow) et intégration de LLM par outils. Tu construis « Deep Atelier », un website builder orienté développeurs dont l'interface principale est un canvas visuel piloté à la fois par la souris et par le prompt, et non un éditeur de texte.

OBJECTIF
Livrer la Phase 1 (MVP web) décrite dans les spécifications jointes : un éditeur où l'on construit une page par glisser-déposer ET par prompt sur un canvas, où chaque action produit du code React + TypeScript + Tailwind propre, et où le code édité à la main se reflète sur le canvas (synchronisation bidirectionnelle).

PRINCIPES NON NÉGOCIABLES
1. Source de vérité unique : une représentation intermédiaire (IR) typée, indépendante de la plateforme, définie en Rust. Le canvas, l'arbre des calques, l'inspecteur, l'éditeur de code et l'IA sont des clients de l'IR. Toute mutation passe par une commande de l'IR (pattern command + undo/redo).
2. Aucun concept DOM/CSS dans l'IR : primitives sémantiques (Box, Stack, Text, Image, Button, Input, Link, ComponentInstance, RawCode) et tokens de style abstraits (spacing.4, color.primary). Le web est un « target » de compilation ; React Native sera le second en v2.
3. IA-first, mais jamais de code brut : le modèle appelle des outils qui sont exactement les commandes de l'IR. Chaque génération est validée par le moteur Rust (schéma, responsive, accessibilité), affichée en aperçu fantôme en streaming, puis acceptée, ajustée ou rejetée. Un prompt accepté = une transaction d'undo.
4. Responsive mobile-first OBLIGATOIRE : chaque propriété de style est un objet par breakpoint { base, sm, md, lg, xl, 2xl }. La vue par défaut du canvas est mobile (390 px). Le code généré utilise les préfixes Tailwind mobile-first. Aucune fonctionnalité, y compris une sortie IA, n'est « terminée » sans test aux largeurs 390 / 768 / 1280 px.
5. Le code exporté appartient au développeur : Next.js App Router, TypeScript strict, Tailwind, zéro dépendance au builder au runtime, lisible comme s'il était écrit à la main.
6. Interface visuelle dominante : le canvas occupe au moins 60 % de l'écran ; le code est un panneau rétractable ; la barre de prompt est toujours à portée (⌘I) et contextuelle à la sélection.

STACK IMPOSÉE
- Monorepo pnpm + Turborepo + workspace Cargo
- Moteur en Rust : crates/ir (schéma, commandes, validation), crates/parser-web (TSX → IR via oxc), crates/compiler-web (IR → TSX + Tailwind), crates/engine-wasm (wasm-bindgen pour l'éditeur). Types exportés vers TypeScript avec ts-rs.
- Backend : Rust (Axum + Tokio) dans crates/api ; Supabase pour Postgres, Auth, Storage.
- apps/editor : Next.js 15 (App Router), React 19, TypeScript strict, Tailwind v4, shadcn/ui, Zustand + Immer, dnd-kit, Monaco
- Canvas : rendu dans une iframe isolée (postMessage) avec overlay de sélection dans le parent
- IA : packages/ai (prompts système, définitions d'outils = commandes IR, évals) + passerelle multi-modèles côté API, Claude par défaut, réponses en streaming
- Collaboration (préparer, activer en Phase 2) : Yjs côté client, Yrs côté serveur
- Tests : cargo test + insta (snapshots du compilateur), Vitest, Playwright (E2E aux 3 breakpoints), évals IA sur un jeu de 30 prompts de référence

MÉTHODE DE TRAVAIL
1. Commence par me proposer : l'arborescence du monorepo, le schéma Rust complet de l'IR, la liste des commandes de mutation et leur définition en tant qu'outils pour le LLM. Attends ma validation.
2. Implémente ensuite dans cet ordre, une étape par PR : (a) crates/ir + tests, (b) compiler-web IR → TSX + snapshots, (c) engine-wasm + canvas en iframe avec sélection/survol, (d) arbre des calques + inspecteur par breakpoint, (e) drag & drop depuis la bibliothèque, (f) couche IA : barre de prompt, outils = commandes, validation, aperçu fantôme, (g) panneau code Monaco, (h) parser-web TSX → IR pour la synchro inverse, (i) export ZIP + push GitHub.
3. À chaque étape : code complet (pas de pseudo-code ni de TODO), tests, et une courte note sur les choix faits et les limites connues.
4. Si une spécification est ambiguë, pose la question au lieu d'inventer.

CRITÈRES DE SUCCÈS DU MVP
- Je construis une landing page (hero, features, pricing, footer) sans écrire de code en moins de 15 minutes, à la souris OU par un seul prompt.
- Toute génération IA est éditable au canvas, s'annule en un ⌘Z, et respecte les 3 breakpoints.
- Le code exporté compile (`pnpm build`) sans erreur, passe ESLint et Lighthouse mobile ≥ 90.
- Je modifie un className dans Monaco : le canvas se met à jour en moins de 300 ms.
- Le rendu est correct à 390, 768 et 1280 px sans scroll horizontal.
- Undo/redo fonctionne sur 100 actions.
```

## Utilisateurs cibles & principes

Le persona principal est le développeur front/full-stack qui veut livrer vite sans perdre le contrôle du code ; les autres en bénéficient sans dicter les choix.

| Persona | Besoin principal | Ce qu'il fait dans Deep Atelier |
| --- | --- | --- |
| Dev full-stack / freelance (principal) | Livrer des sites clients vite, garder un code propre | Maquette au canvas, ajuste dans le code, exporte vers son repo |
| Designer-dev | Passer de la maquette au code sans double travail | Construit au canvas avec les tokens, importe depuis Figma (Phase 3) |
| Lead tech / agence | Standardiser composants et qualité | Publie une bibliothèque de composants et un design system partagés |
| Profil métier (secondaire) | Modifier textes et images sans casser le site | Édite le contenu en mode « Contenu » verrouillé |

**Principes de conception**

- **Visuel d'abord, code toujours.** Tout ce qui se fait au canvas se fait aussi en code, et inversement.
- **Clavier partout.** Command palette (⌘K), raccourcis de type Figma (V, F, T, R) et de type IDE (⌘P, ⌘⇧F).
- **Pas de magie cachée.** Chaque propriété de l'inspecteur affiche la classe Tailwind ou la prop qu'elle produit.
- **Mobile-first par défaut.** Le canvas s'ouvre en 390 px ; on élargit pour surcharger, jamais l'inverse.
- **Réversible.** Undo/redo illimité dans la session, historique de versions persistant, branches Git.

## Interface de l'éditeur

Le canvas occupe le centre et au moins 60 % de l'écran ; tout le reste est un panneau rétractable. C'est ce choix qui distingue Deep Atelier d'un IDE, où le texte occupe le centre.

&#91;embedded content: éditeur en mode Design · 5 zones\]

Le même arbre s'affiche en mobile (cartes empilées) et en desktop (cartes en ligne) ; l'inspecteur montre la classe Tailwind que produit chaque réglage.

**Zones**

- **Barre supérieure** : nom du projet et branche Git, sélecteur de page, sélecteur de breakpoint (390 / 768 / 1024 / 1280 / 1536 + largeur libre), zoom, Preview, Publier, avatars des collaborateurs.
- **Panneau gauche (onglets)** : Calques (arbre de l'IR), Insérer (primitives, sections prêtes, composants du projet), Pages & routes, Assets, Data.
- **Canvas central** : rendu réel dans une iframe ; overlay de sélection, poignées de redimensionnement, guides d'alignement, mesures d'espacement à la Figma ; mode multi-artboards pour voir mobile, tablette et desktop côte à côte.
- **Inspecteur droit** : Layout (flex/grid, gap, padding visuel), Typographie, Couleurs (tokens), Effets, Props du composant, Interactions. Chaque champ montre le breakpoint actif et un point de couleur si la valeur est surchargée à ce breakpoint.
- **Tiroir code inférieur** : Monaco avec le TSX du composant sélectionné, synchronisé avec le canvas ; onglets Terminal (logs de build) et Problèmes (lint, accessibilité).

**Modes**

| Mode | Ce qui est visible | Pour qui |
| --- | --- | --- |
| Design | Canvas + calques + inspecteur, code replié | Construction visuelle |
| Split | Canvas et code côte à côte, sélection liée | Ajustement fin |
| Code | Code en grand, canvas en vignette flottante | Logique, cas complexes |
| Contenu | Textes et images seulement, structure verrouillée | Profil métier |
| Preview | Site interactif, sans UI d'édition | Test et démo |

**Interactions clés** : clic sur un élément du canvas = curseur placé sur la ligne JSX correspondante ; clic sur une ligne JSX = élément sélectionné au canvas ; double-clic sur un texte = édition en place ; ⌘K = palette de commandes, y compris les prompts IA (« ajoute une section témoignages »).

**L'éditeur lui-même** est utilisable sur tablette (≥ 1024 px) en Phase 1. Sur téléphone, seuls la Preview et le mode Contenu sont disponibles ; l'édition complète sur mobile n'est pas un objectif.

## Workflow IA-first

Le prompt est un geste de création principal, à égalité avec le glisser-déposer. L'IA n'écrit jamais de code brut : elle émet des commandes sur l'IR, donc tout ce qu'elle produit reste éditable au canvas, versionné et annulable.

&#91;embedded content: flux IA · prompt → IR, 2 boucles\]

Un rejet efface l'aperçu sans toucher l'IR ; un échec de validation est renvoyé au modèle une fois, puis signalé à l'utilisateur.

**Points d'entrée du prompt**

| Point d'entrée | Portée | Exemple |
| --- | --- | --- |
| Barre de prompt (⌘I) | Projet entier | « Site vitrine pour un cabinet comptable, 5 pages, ton sobre » |
| Prompt sur sélection | Nœud ou section sélectionnés | « Rends cette grille plus aérée » |
| Prompt sur breakpoint | Breakpoint actif uniquement | « En md, passe la navbar en ligne » |
| Multimodal | Capture, croquis, maquette, URL | Glisser une capture : la section est reproduite avec les tokens du projet |
| Agent (panneau latéral) | Tâches multi-étapes, multi-pages | « Ajoute un blog avec liste et page détail, branché sur Supabase » |
| Prompt dans le code | Sélection Monaco | « Extrais ce bloc en composant avec props typées » |

**Règles du moteur IA**

1. **Sorties structurées** : le modèle appelle des outils qui sont les commandes de l'IR (`insertNode`, `setStyle`, `createComponent`…). La logique impossible à modéliser passe par un nœud `RawCode` explicitement marqué.
2. **Contexte ciblé** : IR compacte de la sélection et de ses parents, tokens, catalogue de composants, breakpoint actif. Jamais le projet entier sans nécessité.
3. **Design system d'abord** : l'IA réutilise tokens et composants existants avant d'en créer de nouveaux.
4. **Streaming visible** : les nœuds apparaissent sur le canvas au fil de la génération, en aperçu fantôme.
5. **Revue avant application** : accepter tout, accepter nœud par nœud, ou rejeter. Un prompt accepté = une seule transaction d'undo.
6. **Garde-fous automatiques** : schéma IR, rendu à 390 / 768 / 1280 px sans débordement, contraste, alt-text. Une génération qui casse le responsive n'est jamais appliquée silencieusement.
7. **Historique** : chaque prompt est journalisé, lié à sa version, avec un message de commit généré.
8. **Modèle interchangeable** : passerelle multi-fournisseurs (Claude par défaut), petit modèle pour les retouches simples, cache de contexte, quotas par plan.

**Cibles** : taux d'acceptation des générations ≥ 60 %, premier nœud affiché en moins de 3 s, aucune génération acceptée qui casse le build.

## Fonctionnalités v1 (web)

Le MVP tient en 9 modules « Must » : sans eux, la promesse visuel ⇄ code n'existe pas. Le reste s'ajoute sans toucher au cœur.

| Module | Fonctionnalités | Priorité |
| --- | --- | --- |
| Canvas | Rendu iframe, sélection, survol, multi-sélection, guides, zoom/pan, artboards multi-breakpoints | Must |
| Calques & insertion | Arbre de l'IR, drag & drop, renommage, verrouillage, masquage, bibliothèque de primitives et sections | Must |
| Inspecteur | Layout flex/grid, spacing, typo, couleurs, bordures, effets, tout par breakpoint, affichage de la classe Tailwind produite | Must |
| Responsive | Sélecteur de breakpoint, surcharges par breakpoint, visibilité par breakpoint, alerte de débordement horizontal | Must |
| Code bidirectionnel | Monaco, TSX généré en direct, édition manuelle → mise à jour de l'IR, zones « code libre » préservées | Must |
| Composants | Créer un composant depuis une sélection, props typées, variantes, instances liées | Must |
| Pages & routing | Pages multiples, routes dynamiques simples, layouts partagés, métadonnées SEO par page | Must |
| Export & Git | Export ZIP Next.js, push/pull GitHub, historique de versions | Must |
| Design tokens | Couleurs, typo, espacements, rayons, ombres ; thème clair/sombre ; export tailwind.config | Should |
| Data binding | Connexion REST et Supabase, listes répétées depuis une source, états loading/empty/error | Should |
| Assistant IA | Barre de prompt, prompt sur sélection et breakpoint, multimodal, aperçu fantôme avant application (détail : Workflow IA-first) | Must |
| Qualité | Lint, audit accessibilité (contraste, labels, ordre de focus), score Lighthouse dans l'éditeur | Should |
| Publication | Déploiement en un clic (Vercel, Netlify), domaine personnalisé, preview par branche | Could |
| Collaboration | Multi-curseurs temps réel (Yjs), commentaires sur le canvas, rôles | Could |
| CMS intégré | Collections de contenu avec éditeur pour non-devs | Won't (v1) |
| E-commerce | Panier, paiement (Wave, Orange Money, Stripe) | Won't (v1) |

Les deux « Won't » sont exclus du MVP pour tenir le périmètre ; la Phase 3 les réévalue.

## Responsive mobile-first

Le responsive est une exigence de premier plan, pas une option : tout site produit par Deep Atelier doit être correct à 390 px avant d'exister en desktop. L'IR, l'inspecteur, le canvas et le compilateur sont conçus autour de cette règle.

**Breakpoints (alignés sur Tailwind)**

| Nom | Largeur min | Appareil de référence | Largeur de preview |
| --- | --- | --- | --- |
| base | 0 px | Téléphone | 390 px (défaut du canvas) |
| sm | 640 px | Grand téléphone, paysage | 640 px |
| md | 768 px | Tablette portrait | 768 px |
| lg | 1024 px | Tablette paysage, petit laptop | 1024 px |
| xl | 1280 px | Desktop | 1280 px |
| 2xl | 1536 px | Grand écran | 1536 px |

**Règles du moteur**

1. Chaque propriété de style dans l'IR est `{ base, sm?, md?, lg?, xl?, 2xl? }` ; une valeur non définie hérite du breakpoint inférieur.
2. Le compilateur émet des classes mobile-first : `flex-col md:flex-row gap-4 lg:gap-8`. Jamais de `max-width` media query.
3. Modifier un style quand le canvas est en `lg` crée une surcharge `lg`, sans toucher `base`. L'inspecteur l'indique par un point de couleur et propose « réinitialiser à l'héritage ».
4. Visibilité par breakpoint (`hidden md:flex`) disponible sur tout nœud.
5. Les sections prêtes (hero, pricing, navbar…) sont livrées avec leur comportement mobile : navbar en menu burger sous `md`, grilles qui passent de 1 à 2 à 3 colonnes.

**Garde-fous automatiques** (onglet Problèmes)

- Débordement horizontal détecté à n'importe quel breakpoint → erreur bloquante à la publication.
- Zones tactiles < 44 × 44 px sur `base` → avertissement.
- Texte < 16 px dans un champ de saisie sur `base` (zoom iOS) → avertissement.
- Images sans `sizes`/`srcset` → généré automatiquement via `next/image`.
- Largeur fixe en px supérieure à 390 sur `base` → avertissement avec correction proposée (`w-full max-w-…`).

**Critère de fin** : aucune fonctionnalité n'est livrée sans tests Playwright aux largeurs 390, 768 et 1280 px, et Lighthouse mobile ≥ 90 sur le site exporté de démonstration.

## Architecture technique & modèle de données

Une seule source de vérité, l'IR, et des compilateurs par plateforme : c'est la décision qui rend à la fois la synchro visuel ⇄ code et la v2 mobile possibles.

&#91;embedded content: architecture · 3 entrées, 1 IR, 2 compilateurs\]

Canvas, code et IA écrivent tous dans l'IR ; le compilateur natif (pointillés) se branche en Phase 4 sans toucher au reste.

**Stack**

| Couche | Choix | Raison |
| --- | --- | --- |
| Monorepo | pnpm + Turborepo, workspace Cargo | Packages TS et crates Rust côte à côte |
| Moteur (IR, parser, compilateurs) | Rust → WASM (wasm-bindgen), oxc pour TSX | Même code dans l'éditeur et sur le serveur, synchro < 300 ms |
| Éditeur | Next.js 15, React 19, TypeScript strict | Écosystème, SSR pour le dashboard |
| UI de l'éditeur | Tailwind v4 + shadcn/ui + Radix | Accessibilité, vitesse |
| État | Zustand + Immer, pattern command (commandes définies dans l'IR Rust) | Undo/redo, patches sérialisables |
| Drag & drop | dnd-kit | Contrôle fin, accessible |
| Canvas | iframe isolée + postMessage | Styles du site sans fuite vers l'éditeur, rendu fidèle |
| Code | Monaco, branché sur le parser WASM | AST TSX dans les deux sens |
| Couche IA | Passerelle multi-modèles (Claude par défaut), sorties structurées en commandes IR | Toute génération reste éditable au canvas |
| Collaboration | Yjs côté client, Yrs côté serveur | Temps réel et offline, Phase 2 |
| Backend | Rust (Axum + Tokio) ; Supabase pour Postgres, Auth, Storage | Réutilise la crate IR, RLS côté base |
| Build & preview | WebContainers ou build serveur (esbuild) | Preview de l'app exportée |
| Tests | cargo test + insta (snapshots), Vitest, Playwright | Garantir un code généré stable |

**Choix du langage : Rust plutôt que Go**

Le cœur de Deep Atelier est un compilateur (TSX ⇄ IR ⇄ code) qui doit tourner dans le navigateur pour tenir la synchro sous 300 ms, et sur le serveur pour l'export et la validation des sorties IA. Rust se compile proprement en WASM et dispose des meilleurs parsers TypeScript ; Go non.

| Critère | Rust | Go | Avantage |
| --- | --- | --- | --- |
| Exécution dans le navigateur (WASM) | wasm-bindgen, binaires compacts, pas de GC | Runtime et GC embarqués, binaires de plusieurs Mo ; TinyGo limité | Rust |
| Parsing et génération TSX | oxc et SWC, parsers TS de référence, AST complet | esbuild parse vite mais n'expose pas d'AST manipulable | Rust |
| Collaboration CRDT | Yrs, port officiel de Yjs, compatible avec le client JS | Pas d'implémentation Yjs mature | Rust |
| Un seul moteur partout (éditeur, serveur, compilateur natif v2) | Même crate en WASM et en natif | Impossible côté navigateur | Rust |
| Vitesse de développement | Lente au début (ownership, lifetimes) | Très rapide, langage simple | Go |
| Services réseau (API, files de build, streaming IA) | Axum + Tokio, excellents mais plus verbeux | Idéal, la stdlib suffit souvent | Go |
| Outillage DevOps / cloud-native | Correct | Langage de Kubernetes, Docker, Terraform | Go |

**Verdict**

- **Moteur en Rust** : IR, parser, compilateurs, validation des commandes IA. Compilé en WASM pour l'éditeur, en natif côté serveur ; types partagés avec le front via ts-rs ou specta.
- **Backend applicatif en Rust (Axum)** : deux langages (TypeScript + Rust) au lieu de trois, et la crate IR réutilisée telle quelle.
- **Go reste pertinent côté infra** : CLI de déploiement, opérateurs Kubernetes, outillage DevSecOps, hors du produit.
- **L'UI de l'éditeur reste en TypeScript/React** : Rust n'y apporte rien.

Si l'équipe débute en Rust, garder le backend en TypeScript (route handlers Next.js, Supabase Edge Functions) en Phase 1 et ne passer en Rust que le moteur : c'est là qu'il est irremplaçable.

**Packages**

- `crates/ir` : schéma, commandes de mutation, validation ; types exportés vers TypeScript (ts-rs).
- `crates/compiler-web` : IR → TSX + Tailwind (Next.js).
- `crates/parser-web` : TSX → IR via oxc, avec détection des zones non modélisables.
- `crates/engine-wasm` : bindings WASM du moteur pour l'éditeur.
- `crates/compiler-native` (Phase 4) : IR → React Native / Expo.
- `crates/api` : backend Axum (projets, export, passerelle IA, collaboration Yrs).
- `packages/ai` : prompts système, outils (commandes IR) exposés au modèle, évaluations.
- `packages/tokens` : design tokens → Tailwind config, CSS vars, et plus tard thème RN.
- `apps/editor` : l'éditeur Next.js.

**Modèle de l'IR (extrait)**

```typescript
type Breakpoint = 'base' | 'sm' | 'md' | 'lg' | 'xl' | '2xl';
type Responsive<T> = { base: T } & Partial<Record<Exclude<Breakpoint, 'base'>, T>>;

type NodeType =
  | 'Box' | 'Stack' | 'Grid' | 'Text' | 'Image' | 'Button'
  | 'Link' | 'Input' | 'Icon' | 'Slot' | 'ComponentInstance' | 'RawCode';

interface Node {
  id: string;                       // stable, sert de clé de synchro code ⇄ canvas
  type: NodeType;
  name?: string;                    // nom dans l'arbre des calques
  props: Record<string, PropValue>; // valeurs ou bindings vers une source de données
  style: Partial<StyleMap>;         // tokens abstraits, jamais de classe CSS
  children: string[];
  visibility?: Responsive<boolean>;
  meta?: { locked?: boolean; source?: 'visual' | 'code' };
}

interface StyleMap {
  direction: Responsive<'row' | 'column'>;
  gap: Responsive<SpacingToken>;
  padding: Responsive<BoxSpacing>;
  width: Responsive<SizeValue>;
  fontSize: Responsive<FontToken>;
  color: Responsive<ColorToken>;
  // ...
}

interface Project {
  id: string;
  tokens: DesignTokens;
  components: Record<string, ComponentDef>;
  pages: Page[];                    // route + racine de l'arbre
  dataSources: DataSource[];
  targets: ('web' | 'native')[];    // 'native' activé en Phase 4
}
```

**Synchronisation bidirectionnelle**

1. Action au canvas → commande sur l'IR → compilation incrémentale du seul composant touché → diff appliqué dans Monaco.
2. Frappe dans Monaco → parse en débounce (150 ms) → diff d'AST → commandes sur l'IR → canvas mis à jour.
3. Chaque nœud JSX porte un attribut `data-atl-id` en mode édition (retiré à l'export) pour relier code et canvas.
4. Code non modélisable (hooks, logique, librairies tierces) → nœud `RawCode` : affiché au canvas comme une boîte opaque rendue, éditable uniquement en code, jamais réécrit par le compilateur.

## Préparation du mobile natif (v2)

La v2 génère des apps iOS/Android avec React Native + Expo depuis le même projet. Elle ne coûtera pas une réécriture à condition de respecter dès la v1 les 6 règles ci-dessous.

| Décision v1 | Pourquoi elle sert la v2 |
| --- | --- |
| Primitives sémantiques (`Stack`, `Text`, `Image`) au lieu de balises HTML | Elles se compilent en `View`, `Text`, `Image` RN sans traduction ambiguë |
| Styles en tokens abstraits, layout en flexbox | RN ne connaît ni CSS ni grid : le flex est commun aux deux, la grid web se compile en lignes de `Stack` |
| Aucune logique métier dans les composants générés | Les sources de données et actions sont déclarées dans l'IR, implémentées par adaptateur (fetch web, fetch RN) |
| Navigation décrite comme un graphe de routes dans l'IR | Se compile en App Router côté web et en Expo Router côté mobile, même convention de fichiers |
| Propriétés plateforme marquées `web-only` / `native-only` | Un `hover` ou un `SafeArea` n'est émis que là où il a un sens |
| Champ `targets` dans `Project` dès la v1 | Activer `native` = ajouter un compilateur, pas migrer des données |

**Ce que la v2 ajoute**

- `packages/compiler-native` (IR → Expo Router + React Native, styles via NativeWind pour garder la syntaxe Tailwind).
- Canvas en cadres d'appareils (iPhone, Pixel) avec safe areas et barre de navigation simulées.
- Preview sur téléphone réel via QR code (Expo Go), puis builds EAS pour les stores.
- Composants natifs spécifiques : tab bar, stack navigator, bottom sheet, gestion du clavier.
- Accès aux API de l'appareil (caméra, notifications push, localisation) via des actions déclaratives.

**Hors périmètre** : Flutter, Swift/Kotlin natif, partage de code avec des apps existantes non créées dans Deep Atelier.

## Roadmap & critères d'acceptation

Quatre phases, chacune ouverte par une porte mesurable ; le mobile natif arrive en Phase 4, environ 34 semaines après le démarrage, sans réécriture.

&#91;embedded content: roadmap · 4 phases, 4 portes\]

Les durées sont des estimations à recaler après la Phase 1 ; les portes, elles, ne se négocient pas.

**Checklist d'acceptation du MVP (Phase 1)**

- [ ] Une landing page (hero, features, pricing, footer) se construit sans écrire de code en moins de 15 minutes
- [ ] La même landing se génère par un seul prompt, puis s'ajuste au canvas sans toucher au code
- [ ] Toute génération IA passe par des commandes IR et s'annule en un seul ⌘Z
- [ ] Le canvas s'ouvre en 390 px et le rendu est correct à 390, 768 et 1280 px sans scroll horizontal
- [ ] Modifier un style en `lg` crée une surcharge sans toucher `base`
- [ ] Une classe modifiée dans Monaco met à jour le canvas en moins de 300 ms
- [ ] Un clic au canvas place le curseur sur la bonne ligne JSX, et inversement
- [ ] Le code exporté passe `pnpm build`, ESLint et TypeScript strict sans erreur
- [ ] Le site exporté obtient Lighthouse mobile ≥ 90 (performance et accessibilité)
- [ ] Aucune dépendance à Deep Atelier dans le `package.json` exporté
- [ ] Undo/redo fiable sur 100 actions consécutives
- [ ] Le schéma de l'IR contient déjà `targets` et les marqueurs `web-only` / `native-only`
