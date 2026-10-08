-- Projets, document courant et versions (ADR 0001 § 9 et § 16).
--
-- Seule l'API Rust écrit dans ces tables, avec le rôle propriétaire des tables, après avoir
-- validé l'IR. Les rôles `anon` et `authenticated` n'ont aucun droit d'écriture ; un utilisateur
-- connecté peut seulement lire ses propres lignes (RLS), en défense en profondeur.

create table public.projects (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users (id) on delete cascade,
  name text not null check (char_length(btrim(name)) between 1 and 120),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

comment on table public.projects is 'Projet Deep Atelier ; écrit seulement par l''API.';

create index projects_owner_id_updated_at_idx on public.projects (owner_id, updated_at desc);

-- Document courant : l'IR sérialisée, et sa version pour la concurrence optimiste (chaque
-- sauvegarde automatique envoie la version qu'elle remplace).
create table public.project_documents (
  project_id uuid primary key references public.projects (id) on delete cascade,
  version bigint not null check (version >= 1),
  ir_version integer not null check (ir_version >= 1),
  document jsonb not null,
  updated_at timestamptz not null default now(),
  updated_by uuid references auth.users (id) on delete set null
);

comment on table public.project_documents is 'IR courante d''un projet ; écrite seulement par l''API.';

create index project_documents_updated_by_idx on public.project_documents (updated_by);

-- Versions : instantanés numérotés par projet. Une version `user` ou `ai` porte un message (et un
-- nom facultatif) ; une version `restore` garde l'état d'avant la restauration de la version
-- `restored_from`, l'interface en compose le libellé.
create table public.document_versions (
  id uuid primary key default gen_random_uuid(),
  project_id uuid not null references public.projects (id) on delete cascade,
  number integer not null check (number >= 1),
  name text check (name is null or char_length(btrim(name)) between 1 and 120),
  message text check (message is null or char_length(btrim(message)) between 1 and 2000),
  origin text not null check (origin in ('user', 'ai', 'restore')),
  restored_from integer check (restored_from >= 1),
  -- Version du document courant au moment de l'instantané.
  document_version bigint not null check (document_version >= 1),
  ir_version integer not null check (ir_version >= 1),
  document jsonb not null,
  created_at timestamptz not null default now(),
  created_by uuid references auth.users (id) on delete set null,
  unique (project_id, number),
  check ((origin = 'restore') = (restored_from is not null)),
  check (origin = 'restore' or message is not null)
);

comment on table public.document_versions is 'Versions d''un projet ; écrites seulement par l''API.';

create index document_versions_created_by_idx on public.document_versions (created_by);

-- Droits : lecture seule pour les utilisateurs connectés, rien pour les anonymes.
revoke all on table public.projects, public.project_documents, public.document_versions
  from anon, authenticated;
grant select on table public.projects, public.project_documents, public.document_versions
  to authenticated;

alter table public.projects enable row level security;
alter table public.project_documents enable row level security;
alter table public.document_versions enable row level security;

create policy projects_select_own on public.projects
  for select to authenticated
  using (owner_id = (select auth.uid()));

create policy project_documents_select_own on public.project_documents
  for select to authenticated
  using (
    exists (
      select 1 from public.projects p
      where p.id = project_documents.project_id and p.owner_id = (select auth.uid())
    )
  );

create policy document_versions_select_own on public.document_versions
  for select to authenticated
  using (
    exists (
      select 1 from public.projects p
      where p.id = document_versions.project_id and p.owner_id = (select auth.uid())
    )
  );
