# Atelier

Website builder orienté développeurs : le canvas visuel est l'interface principale, et chaque
action — à la souris ou par prompt — produit du code React + TypeScript + Tailwind lisible,
versionné et exportable. Le code édité à la main se reflète sur le canvas.

> La vitesse d'un outil visuel, la liberté d'un IDE, sans lock-in.

## État

Phase 1 (MVP web) : architecture acceptée, prochaine étape (a) `crates/ir`.

- Spécifications : [`docs/SPEC.md`](docs/SPEC.md)
- Architecture de la Phase 1 : [`docs/adr/0001-architecture-phase1.md`](docs/adr/0001-architecture-phase1.md)

## Stack

- Monorepo pnpm + Turborepo + workspace Cargo
- Moteur en Rust (IR, compilateur web, parser TSX via oxc), compilé en WebAssembly pour l'éditeur
- Backend Rust (Axum + Tokio), Supabase (Postgres, Auth, Storage)
- Éditeur : Next.js 15, React 19, TypeScript strict, Tailwind v4, shadcn/ui, Zustand, dnd-kit, Monaco
- IA : outils = commandes de l'IR, Claude par défaut

## Feuille de route de la Phase 1

- [ ] (a) `crates/ir` : schéma, commandes, historique, validation
- [ ] (b) `crates/compiler-web` : IR → TSX + Tailwind
- [ ] (b2) `crates/api` + Supabase : auth, projets, sauvegarde, versions
- [ ] (c) `crates/engine-wasm` + canvas en iframe
- [ ] (d) Calques + inspecteur par breakpoint
- [ ] (e) Glisser-déposer depuis la bibliothèque
- [ ] (f) Couche IA : prompt, outils, validation, aperçu fantôme
- [ ] (g) Panneau code Monaco
- [ ] (h) `crates/parser-web` : synchro TSX → IR
- [ ] (i) Export ZIP + push GitHub
