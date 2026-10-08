# Deep Atelier

Website builder orienté développeurs : le canvas visuel est l'interface principale, et chaque
action — à la souris ou par prompt — produit du code React + TypeScript + Tailwind lisible,
versionné et exportable. Le code édité à la main se reflète sur le canvas.

> La vitesse d'un outil visuel, la liberté d'un IDE, sans lock-in.

## État

Phase 1 (MVP web) : étapes (a) à (c) livrées (IR, compilateur web, API + Supabase, moteur wasm,
canvas et coquille de l'éditeur), prochaine étape (d) : calques et inspecteur.

- Spécifications : [`docs/SPEC.md`](docs/SPEC.md)
- Architecture de la Phase 1 : [`docs/adr/0001-architecture-phase1.md`](docs/adr/0001-architecture-phase1.md)

## Développement

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build -p deep-atelier-ir --target wasm32-unknown-unknown
```

API et Supabase en local (Docker et CLI Supabase) : la clé de signature des jetons se crée une
fois, puis le stack démarre avec les migrations de `supabase/migrations`.

```sh
echo '[]' > supabase/signing_keys.json && supabase gen signing-key --algorithm ES256 --yes
supabase start
cp -n .env.example .env
cargo run -p deep-atelier-api
# Tests de l'API, intégration comprise
set -a; eval "$(supabase status -o env)"; set +a
cargo test -p deep-atelier-api -- --include-ignored
```

### Lancer Deep Atelier en local

Prérequis : Docker Desktop démarré, CLI Supabase, Rust (rustup), Node 22.19+ et pnpm 12.

Une seule fois :

```sh
cargo install wasm-bindgen-cli --version 0.2.129 --locked
pnpm install
echo '[]' > supabase/signing_keys.json && supabase gen signing-key --algorithm ES256 --yes
cp -n .env.example .env                                  # API : base et Supabase locaux
cp -n apps/editor/.env.example apps/editor/.env.local
# Dans apps/editor/.env.local : NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY = PUBLISHABLE_KEY de `supabase status`
```

Puis, à chaque session de travail, trois terminaux :

```sh
supabase start                                           # 1. Postgres, Auth, Mailpit (migrations appliquées)
cargo run -p deep-atelier-api                            # 2. API sur http://127.0.0.1:3001
pnpm turbo run build --filter='@deep-atelier/editor^...' # 3. moteur wasm et runtime du canvas,
pnpm --filter @deep-atelier/editor dev                   #    puis l'éditeur sur http://localhost:3000
```

Ouvrir http://localhost:3000 et saisir une adresse e-mail : le lien de connexion arrive dans Mailpit
(http://127.0.0.1:54324). La connexion GitHub demande une OAuth App (voir `supabase/config.toml`).

- Port 3000 occupé : `PORT=3100 pnpm --filter @deep-atelier/editor dev`, et l'API doit autoriser
  cette origine : `CORS_ORIGINS=http://localhost:3100 cargo run -p deep-atelier-api`.
- Après un changement du Rust (`crates/`) ou du runtime du canvas, relancer la commande 3 ; après un
  changement des types Rust exportés, `cargo xtask codegen`.
- Ne pas lancer `pnpm build` pendant que l'éditeur tourne en `dev` : Next.js 15 partage `.next`
  entre les deux (erreurs 500). Dans ce cas, supprimer `apps/editor/.next` et relancer `dev`.

Tests de l'éditeur : vitest, puis Playwright aux largeurs 390, 768 et 1280 (serveurs démarrés).

```sh
pnpm test
E2E_BASE_URL=http://localhost:3000 pnpm --filter @deep-atelier/editor e2e
```

Vérifier un projet exporté comme la CI (job `export`) : la landing de démonstration est écrite dans
`$OUT`, construite, servie, puis contrôlée (viewports, menu, Lighthouse mobile) avec Chrome.

```sh
OUT=/tmp/deep-atelier-landing
DEEP_ATELIER_EXPORT_DIR=$OUT cargo test -p deep-atelier-compiler-web --test demo
(cd $OUT && pnpm dlx prettier@3.9.9 --check . && pnpm install && pnpm build && pnpm lint)
(cd $OUT && pnpm start --port 3913) &
pnpm install                           # installe aussi tools/export-check (workspace)
CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  node tools/export-check/check.mjs --url http://localhost:3913 --paths /,/a-propos --menu Menu --lighthouse
```

## Stack

- Monorepo pnpm + Turborepo + workspace Cargo
- Moteur en Rust (IR, compilateur web, parser TSX via oxc), compilé en WebAssembly pour l'éditeur
- Backend Rust (Axum + Tokio), Supabase (Postgres, Auth, Storage)
- Éditeur : Next.js 15, React 19, TypeScript strict, Tailwind v4, shadcn/ui, Zustand, dnd-kit, Monaco,
  next-intl (français et anglais)
- Code exporté : Next.js 16 (App Router), React 19, TypeScript strict, Tailwind v4, mis en forme
  comme Prettier
- IA : outils = commandes de l'IR, Claude par défaut

## Feuille de route de la Phase 1

- [x] (a) `crates/ir` : schéma, commandes, historique, validation
- [x] (b) `crates/compiler-web` : IR → TSX + Tailwind
- [x] (b2) `crates/api` + Supabase : auth, projets, sauvegarde, versions
- [x] (c) `crates/engine-wasm` + canvas en iframe
- [ ] (d) Calques + inspecteur par breakpoint
- [ ] (e) Glisser-déposer depuis la bibliothèque
- [ ] (f) Couche IA : prompt, outils, validation, aperçu fantôme
- [ ] (g) Panneau code Monaco
- [ ] (h) `crates/parser-web` : synchro TSX → IR
- [ ] (i) Export ZIP + push GitHub
