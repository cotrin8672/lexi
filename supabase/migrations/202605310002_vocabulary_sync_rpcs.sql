-- Vocabulary sync RPCs for Lexi desktop push/pull.

create or replace function public.apply_vocabulary_mutation(envelope jsonb)
returns jsonb
language plpgsql
security invoker
set search_path = public
as $$
declare
  v_user_id uuid := auth.uid();
  v_operation_id uuid;
  v_mutation_type text;
  v_payload jsonb;
  v_existing_revision bigint;
  v_existing_status text;
  v_lexeme_id uuid;
  v_snapshot_id uuid;
  v_revision bigint;
  v_language text;
  v_canonical_text text;
  v_canonical_key text;
  v_result_language text;
  v_schema_version text;
  v_provider text;
  v_model text;
  v_content jsonb;
  v_part_of_speech text;
  v_form jsonb;
  v_form_text text;
  v_form_key text;
  v_relation text;
  v_source text;
  v_form_id uuid;
begin
  if v_user_id is null then
    raise exception 'not authenticated';
  end if;

  if not public.lexi_is_admin() then
    raise exception 'admin access required';
  end if;

  perform pg_advisory_xact_lock(hashtext('lexi:vocabulary_sync:' || v_user_id::text));

  v_operation_id := nullif(envelope->>'operationId', '')::uuid;
  v_mutation_type := envelope->>'mutationType';
  v_payload := envelope->'payload';

  if v_operation_id is null or v_mutation_type is null or v_payload is null then
    raise exception 'invalid mutation envelope';
  end if;

  select vm.server_revision, vm.status
  into v_existing_revision, v_existing_status
  from public.vocabulary_mutations vm
  where vm.user_id = v_user_id
    and vm.operation_id = v_operation_id;

  if found then
    select ul.id
    into v_lexeme_id
    from public.user_lexemes ul
    where ul.user_id = v_user_id
      and ul.language = coalesce(v_payload->>'language', 'en')
      and ul.canonical_key = v_payload->>'canonicalKey'
      and ul.deleted_at is null
    limit 1;

    select cs.id
    into v_snapshot_id
    from public.card_snapshots cs
    where cs.user_id = v_user_id
      and cs.lexeme_id = v_lexeme_id
      and cs.result_language = v_payload->>'resultLanguage'
      and cs.active = true
    order by cs.created_at desc
    limit 1;

    return jsonb_build_object(
      'operationId', v_operation_id::text,
      'serverRevision', v_existing_revision,
      'status', v_existing_status,
      'lexemeId', v_lexeme_id::text,
      'cardSnapshotId', v_snapshot_id::text
    );
  end if;

  if v_mutation_type <> 'save_card_snapshot' then
    raise exception 'unsupported mutation type: %', v_mutation_type;
  end if;

  v_language := coalesce(v_payload->>'language', 'en');
  v_canonical_text := v_payload->>'canonicalText';
  v_canonical_key := v_payload->>'canonicalKey';
  v_result_language := v_payload->>'resultLanguage';
  v_schema_version := v_payload->>'schemaVersion';
  v_provider := v_payload->>'provider';
  v_model := v_payload->>'model';
  v_content := v_payload->'content';

  if v_canonical_text is null
    or v_canonical_key is null
    or v_result_language is null
    or v_schema_version is null
    or v_content is null then
    raise exception 'invalid save_card_snapshot payload';
  end if;

  v_part_of_speech := v_content->'translations'->0->>'note';

  insert into public.user_lexemes (
    user_id,
    language,
    canonical_text,
    canonical_key,
    part_of_speech
  ) values (
    v_user_id,
    v_language,
    v_canonical_text,
    v_canonical_key,
    v_part_of_speech
  )
  on conflict (user_id, language, canonical_key)
  do update set
    canonical_text = excluded.canonical_text,
    part_of_speech = coalesce(excluded.part_of_speech, public.user_lexemes.part_of_speech),
    updated_at = now(),
    deleted_at = null
  returning id into v_lexeme_id;

  insert into public.vocabulary_changes (
    user_id,
    operation_id,
    entity_type,
    entity_id,
    change_type,
    payload
  ) values (
    v_user_id,
    v_operation_id,
    'user_lexeme',
    v_lexeme_id,
    'upsert',
    jsonb_build_object(
      'language', v_language,
      'canonicalText', v_canonical_text,
      'canonicalKey', v_canonical_key,
      'partOfSpeech', v_part_of_speech
    )
  )
  returning server_revision into v_revision;

  if jsonb_typeof(v_payload->'forms') = 'array' then
    for v_form in select * from jsonb_array_elements(v_payload->'forms')
    loop
      v_form_text := v_form->>'formText';
      v_form_key := coalesce(v_form->>'formKey', lower(trim(v_form_text)));
      v_relation := coalesce(v_form->>'relation', 'observed');
      v_source := coalesce(v_form->>'source', 'sync');

      if v_form_text is null or v_form_key is null then
        continue;
      end if;

      insert into public.lexeme_forms (
        user_id,
        lexeme_id,
        language,
        form_text,
        form_key,
        relation,
        source,
        confidence
      ) values (
        v_user_id,
        v_lexeme_id,
        v_language,
        v_form_text,
        v_form_key,
        v_relation,
        v_source,
        1.0
      )
      on conflict (user_id, language, form_key, lexeme_id, relation)
      do update set
        form_text = excluded.form_text,
        source = excluded.source,
        confidence = excluded.confidence
      returning id into v_form_id;

      insert into public.vocabulary_changes (
        user_id,
        operation_id,
        entity_type,
        entity_id,
        change_type,
        payload
      ) values (
        v_user_id,
        v_operation_id,
        'lexeme_form',
        v_form_id,
        'upsert',
        jsonb_build_object(
          'lexemeId', v_lexeme_id::text,
          'lexemeFormId', v_form_id::text,
          'language', v_language,
          'formText', v_form_text,
          'formKey', v_form_key,
          'relation', v_relation,
          'source', v_source
        )
      )
      returning server_revision into v_revision;
    end loop;
  end if;

  update public.card_snapshots
  set active = false
  where user_id = v_user_id
    and lexeme_id = v_lexeme_id
    and result_language = v_result_language
    and active = true;

  insert into public.card_snapshots (
    user_id,
    lexeme_id,
    schema_version,
    provider,
    model,
    result_language,
    content,
    active
  ) values (
    v_user_id,
    v_lexeme_id,
    v_schema_version,
    v_provider,
    v_model,
    v_result_language,
    v_content,
    true
  )
  returning id into v_snapshot_id;

  insert into public.vocabulary_changes (
    user_id,
    operation_id,
    entity_type,
    entity_id,
    change_type,
    payload
  ) values (
    v_user_id,
    v_operation_id,
    'card_snapshot',
    v_snapshot_id,
    'upsert',
    jsonb_build_object(
      'lexemeId', v_lexeme_id::text,
      'cardSnapshotId', v_snapshot_id::text,
      'language', v_language,
      'canonicalText', v_canonical_text,
      'canonicalKey', v_canonical_key,
      'resultLanguage', v_result_language,
      'schemaVersion', v_schema_version,
      'provider', v_provider,
      'model', v_model,
      'content', v_content,
      'forms', coalesce(v_payload->'forms', '[]'::jsonb)
    )
  )
  returning server_revision into v_revision;

  insert into public.vocabulary_mutations (
    user_id,
    operation_id,
    mutation_type,
    payload,
    status,
    server_revision
  ) values (
    v_user_id,
    v_operation_id,
    v_mutation_type,
    v_payload,
    'accepted',
    v_revision
  );

  return jsonb_build_object(
    'operationId', v_operation_id::text,
    'serverRevision', v_revision,
    'status', 'accepted',
    'lexemeId', v_lexeme_id::text,
    'cardSnapshotId', v_snapshot_id::text
  );
end;
$$;

create or replace function public.pull_vocabulary_changes(
  since_revision bigint default 0,
  batch_limit integer default 100
)
returns jsonb
language plpgsql
security invoker
set search_path = public
as $$
declare
  v_user_id uuid := auth.uid();
  v_changes jsonb;
  v_last_revision bigint;
begin
  if v_user_id is null then
    raise exception 'not authenticated';
  end if;

  if not public.lexi_is_admin() then
    raise exception 'admin access required';
  end if;

  if batch_limit is null or batch_limit < 1 then
    batch_limit := 100;
  end if;

  if batch_limit > 500 then
    batch_limit := 500;
  end if;

  select coalesce(
    jsonb_agg(
      jsonb_build_object(
        'serverRevision', vc.server_revision,
        'operationId', vc.operation_id::text,
        'entityType', vc.entity_type,
        'entityId', vc.entity_id::text,
        'changeType', vc.change_type,
        'payload', vc.payload,
        'createdAt', vc.created_at
      )
      order by vc.server_revision asc
    ),
    '[]'::jsonb
  )
  into v_changes
  from (
    select *
    from public.vocabulary_changes vc
    where vc.user_id = v_user_id
      and vc.server_revision > coalesce(since_revision, 0)
    order by vc.server_revision asc
    limit batch_limit
  ) vc;

  select coalesce(max((change->>'serverRevision')::bigint), coalesce(since_revision, 0))
  into v_last_revision
  from jsonb_array_elements(v_changes) as change;

  return jsonb_build_object(
    'changes', v_changes,
    'lastRevision', v_last_revision
  );
end;
$$;

grant execute on function public.apply_vocabulary_mutation(jsonb) to authenticated;
grant execute on function public.pull_vocabulary_changes(bigint, integer) to authenticated;
