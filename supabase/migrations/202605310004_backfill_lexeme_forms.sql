-- One-time repair for lexeme_forms gaps on existing Supabase vocabulary data.
-- Idempotent: safe to re-run after partial deploy.

-- Canonical alias for every live lexeme.
insert into public.lexeme_forms (
  user_id,
  lexeme_id,
  language,
  form_text,
  form_key,
  relation,
  source,
  confidence
)
select
  ul.user_id,
  ul.id,
  ul.language,
  ul.canonical_text,
  ul.canonical_key,
  'canonical',
  'repair',
  1.0
from public.user_lexemes ul
where ul.deleted_at is null
on conflict (user_id, language, form_key, lexeme_id, relation)
do update set
  form_text = excluded.form_text,
  source = excluded.source,
  confidence = excluded.confidence;

-- Irregular aliases from active card inflections.
insert into public.lexeme_forms (
  user_id,
  lexeme_id,
  language,
  form_text,
  form_key,
  relation,
  source,
  confidence
)
select distinct
  cs.user_id,
  cs.lexeme_id,
  ul.language,
  inflection.form_text,
  lower(trim(inflection.form_text)) as form_key,
  'irregular',
  'repair',
  1.0
from public.card_snapshots cs
inner join public.user_lexemes ul on ul.id = cs.lexeme_id
cross join lateral (
  select nullif(trim(value->>'form'), '') as form_text
  from jsonb_array_elements(cs.content->'inflections') as value
) inflection
where cs.active = true
  and ul.deleted_at is null
  and inflection.form_text is not null
on conflict (user_id, language, form_key, lexeme_id, relation)
do update set
  form_text = excluded.form_text,
  source = excluded.source,
  confidence = excluded.confidence;
