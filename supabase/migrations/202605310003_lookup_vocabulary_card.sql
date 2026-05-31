-- On-demand vocabulary card lookup for Lexi desktop cache miss path.

create or replace function public.lookup_vocabulary_card(
  lookup_key text,
  result_language text,
  language text default 'en'
)
returns jsonb
language plpgsql
security invoker
set search_path = public
as $$
declare
  v_user_id uuid := auth.uid();
  v_matches jsonb;
begin
  if v_user_id is null then
    raise exception 'not authenticated';
  end if;

  if not public.lexi_is_admin() then
    raise exception 'admin access required';
  end if;

  if lookup_key is null or trim(lookup_key) = '' or result_language is null then
    return jsonb_build_object('matches', '[]'::jsonb);
  end if;

  select coalesce(
    jsonb_agg(
      jsonb_build_object(
        'lexemeId', match_row.lexeme_id::text,
        'relation', match_row.relation,
        'formText', match_row.form_text,
        'content', match_row.content,
        'language', match_row.language,
        'canonicalText', match_row.canonical_text,
        'canonicalKey', match_row.canonical_key,
        'resultLanguage', match_row.result_language,
        'schemaVersion', match_row.schema_version,
        'provider', match_row.provider,
        'model', match_row.model,
        'operationId', match_row.operation_id::text,
        'serverRevision', match_row.server_revision,
        'forms', match_row.forms
      )
      order by match_row.relation, match_row.form_text
    ),
    '[]'::jsonb
  )
  into v_matches
  from (
    with active_cards as (
      select distinct on (cs.lexeme_id)
        cs.lexeme_id,
        cs.content,
        cs.schema_version,
        cs.provider,
        cs.model,
        cs.result_language,
        cs.created_at
      from public.card_snapshots cs
      where cs.user_id = v_user_id
        and cs.result_language = lookup_vocabulary_card.result_language
        and cs.active = true
      order by cs.lexeme_id, cs.created_at desc
    ),
    form_matches as (
      select
        lf.lexeme_id,
        lf.relation,
        lf.form_text
      from public.lexeme_forms lf
      inner join public.user_lexemes ul on ul.id = lf.lexeme_id
      where lf.user_id = v_user_id
        and ul.user_id = v_user_id
        and lf.language = lookup_vocabulary_card.language
        and lf.form_key = lookup_key
        and ul.deleted_at is null

      union all

      select
        ul.id as lexeme_id,
        'canonical'::text as relation,
        ul.canonical_text as form_text
      from public.user_lexemes ul
      where ul.user_id = v_user_id
        and ul.language = lookup_vocabulary_card.language
        and ul.canonical_key = lookup_key
        and ul.deleted_at is null
    )
    select
      fm.lexeme_id,
      fm.relation,
      fm.form_text,
      ac.content,
      ul.language,
      ul.canonical_text,
      ul.canonical_key,
      ac.result_language,
      ac.schema_version,
      ac.provider,
      ac.model,
      vc.operation_id,
      vc.server_revision,
      coalesce(
        (
          select jsonb_agg(
            jsonb_build_object(
              'formText', lf.form_text,
              'formKey', lf.form_key,
              'relation', lf.relation,
              'source', lf.source
            )
            order by lf.relation, lf.form_text
          )
          from public.lexeme_forms lf
          where lf.user_id = v_user_id
            and lf.lexeme_id = fm.lexeme_id
            and lf.language = lookup_vocabulary_card.language
        ),
        '[]'::jsonb
      ) as forms
    from form_matches fm
    inner join public.user_lexemes ul on ul.id = fm.lexeme_id
    inner join active_cards ac on ac.lexeme_id = fm.lexeme_id
    left join lateral (
      select
        vc.operation_id,
        vc.server_revision
      from public.vocabulary_changes vc
      where vc.user_id = v_user_id
        and vc.entity_type = 'card_snapshot'
        and (vc.payload->>'lexemeId')::uuid = fm.lexeme_id
        and vc.payload->>'resultLanguage' = lookup_vocabulary_card.result_language
      order by vc.server_revision desc
      limit 1
    ) vc on true
    where ul.user_id = v_user_id
      and ul.deleted_at is null
  ) match_row;

  return jsonb_build_object('matches', v_matches);
end;
$$;

grant execute on function public.lookup_vocabulary_card(text, text, text) to authenticated;
