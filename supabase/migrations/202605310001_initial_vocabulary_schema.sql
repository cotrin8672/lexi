-- Initial Lexi vocabulary schema.
-- Personal-use phase: only authenticated users whose JWT app_metadata.admin is true
-- can read or write these tables. User-owned rows are still scoped by auth.uid()
-- so the policy can later relax to auth.uid() = user_id without reshaping data.

create or replace function public.lexi_is_admin()
returns boolean
language sql
stable
as $$
  select auth.role() = 'authenticated'
    and coalesce(auth.jwt() -> 'app_metadata' ->> 'admin', 'false') = 'true';
$$;

create table if not exists public.dictionary_sources (
  id uuid primary key default gen_random_uuid(),
  source_key text not null unique,
  display_name text not null,
  license text,
  version text,
  imported_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists public.dictionary_entries (
  id uuid primary key default gen_random_uuid(),
  source_id uuid not null references public.dictionary_sources(id) on delete restrict,
  language text not null,
  headword text not null,
  normalized_key text not null,
  reading text,
  part_of_speech text,
  definitions jsonb not null default '[]'::jsonb,
  metadata jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (source_id, language, normalized_key)
);

create table if not exists public.user_lexemes (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid(),
  language text not null,
  canonical_text text not null,
  canonical_key text not null,
  part_of_speech text,
  dictionary_entry_id uuid references public.dictionary_entries(id) on delete set null,
  favorite boolean not null default false,
  user_note text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  deleted_at timestamptz,
  unique (user_id, language, canonical_key)
);

create table if not exists public.lexeme_forms (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid(),
  lexeme_id uuid not null references public.user_lexemes(id) on delete cascade,
  language text not null,
  form_text text not null,
  form_key text not null,
  relation text not null,
  source text not null,
  confidence real,
  created_at timestamptz not null default now(),
  unique (user_id, language, form_key, lexeme_id, relation)
);

create table if not exists public.card_snapshots (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid(),
  lexeme_id uuid not null references public.user_lexemes(id) on delete cascade,
  schema_version text not null,
  provider text,
  model text,
  result_language text not null,
  content jsonb not null,
  active boolean not null default true,
  created_at timestamptz not null default now()
);

create table if not exists public.lookup_events (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid(),
  operation_id uuid not null,
  lexeme_id uuid references public.user_lexemes(id) on delete set null,
  language text not null,
  lookup_key text not null,
  result_mode text not null,
  capture_method text,
  created_at timestamptz not null default now(),
  unique (user_id, operation_id)
);

create table if not exists public.vocabulary_mutations (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid(),
  operation_id uuid not null,
  mutation_type text not null,
  payload jsonb not null,
  status text not null default 'accepted',
  server_revision bigint,
  created_at timestamptz not null default now(),
  unique (user_id, operation_id)
);

create table if not exists public.vocabulary_changes (
  server_revision bigint generated always as identity primary key,
  user_id uuid not null default auth.uid(),
  operation_id uuid not null,
  entity_type text not null,
  entity_id uuid,
  change_type text not null,
  payload jsonb not null,
  created_at timestamptz not null default now(),
  unique (user_id, operation_id, entity_type, entity_id, change_type)
);

create index if not exists idx_dictionary_entries_lookup
  on public.dictionary_entries (language, normalized_key);

create index if not exists idx_user_lexemes_lookup
  on public.user_lexemes (user_id, language, canonical_key)
  where deleted_at is null;

create index if not exists idx_lexeme_forms_lookup
  on public.lexeme_forms (user_id, language, form_key);

create index if not exists idx_card_snapshots_active
  on public.card_snapshots (user_id, lexeme_id, created_at desc)
  where active;

create index if not exists idx_vocabulary_changes_pull
  on public.vocabulary_changes (user_id, server_revision);

alter table public.dictionary_sources enable row level security;
alter table public.dictionary_entries enable row level security;
alter table public.user_lexemes enable row level security;
alter table public.lexeme_forms enable row level security;
alter table public.card_snapshots enable row level security;
alter table public.lookup_events enable row level security;
alter table public.vocabulary_mutations enable row level security;
alter table public.vocabulary_changes enable row level security;

create policy dictionary_sources_admin_all
on public.dictionary_sources
for all
to authenticated
using (public.lexi_is_admin())
with check (public.lexi_is_admin());

create policy dictionary_entries_admin_all
on public.dictionary_entries
for all
to authenticated
using (public.lexi_is_admin())
with check (public.lexi_is_admin());

create policy user_lexemes_admin_owner_all
on public.user_lexemes
for all
to authenticated
using (public.lexi_is_admin() and user_id = auth.uid())
with check (public.lexi_is_admin() and user_id = auth.uid());

create policy lexeme_forms_admin_owner_all
on public.lexeme_forms
for all
to authenticated
using (public.lexi_is_admin() and user_id = auth.uid())
with check (public.lexi_is_admin() and user_id = auth.uid());

create policy card_snapshots_admin_owner_all
on public.card_snapshots
for all
to authenticated
using (public.lexi_is_admin() and user_id = auth.uid())
with check (public.lexi_is_admin() and user_id = auth.uid());

create policy lookup_events_admin_owner_all
on public.lookup_events
for all
to authenticated
using (public.lexi_is_admin() and user_id = auth.uid())
with check (public.lexi_is_admin() and user_id = auth.uid());

create policy vocabulary_mutations_admin_owner_all
on public.vocabulary_mutations
for all
to authenticated
using (public.lexi_is_admin() and user_id = auth.uid())
with check (public.lexi_is_admin() and user_id = auth.uid());

create policy vocabulary_changes_admin_owner_all
on public.vocabulary_changes
for all
to authenticated
using (public.lexi_is_admin() and user_id = auth.uid())
with check (public.lexi_is_admin() and user_id = auth.uid());
