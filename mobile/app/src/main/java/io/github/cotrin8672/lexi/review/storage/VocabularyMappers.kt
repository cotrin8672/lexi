package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.schema.CardSnapshot
import io.github.cotrin8672.lexi.review.schema.LexemeForm
import io.github.cotrin8672.lexi.review.schema.LexiResultV1
import io.github.cotrin8672.lexi.review.schema.UserLexeme
import io.github.cotrin8672.lexi.review.schema.VocabularyBundle
import io.github.cotrin8672.lexi.review.storage.entity.CachedCardSnapshotEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedLexemeFormEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedUserLexemeEntity
import kotlinx.serialization.json.Json

private val vocabularyJson = Json {
    ignoreUnknownKeys = true
    isLenient = true
}

fun VocabularyBundle.toCacheEntities(updatedAt: String): VocabularyCacheEntities =
    VocabularyCacheEntities(
        lexemes = lexemes.map { it.toEntity(updatedAt) },
        snapshots = snapshots.map { it.toEntity(updatedAt) },
        forms = forms.map { it.toEntity(updatedAt) },
    )

data class VocabularyCacheEntities(
    val lexemes: List<CachedUserLexemeEntity>,
    val snapshots: List<CachedCardSnapshotEntity>,
    val forms: List<CachedLexemeFormEntity>,
)

fun CachedUserLexemeEntity.toDomain(): UserLexeme = UserLexeme(
    id = id,
    userId = userId,
    language = language,
    canonicalText = canonicalText,
    canonicalKey = canonicalKey,
    partOfSpeech = partOfSpeech,
)

fun CachedCardSnapshotEntity.toDomain(): CardSnapshot = CardSnapshot(
    id = id,
    userId = userId,
    lexemeId = lexemeId,
    schemaVersion = schemaVersion,
    provider = provider,
    model = model,
    resultLanguage = resultLanguage,
    content = vocabularyJson.decodeFromString<LexiResultV1>(contentJson),
    active = active,
    createdAt = createdAt,
)

fun CachedLexemeFormEntity.toDomain(): LexemeForm = LexemeForm(
    id = id,
    userId = userId,
    lexemeId = lexemeId,
    language = language,
    formText = formText,
    formKey = formKey,
    relation = relation,
    source = source,
)

private fun UserLexeme.toEntity(updatedAt: String): CachedUserLexemeEntity =
    CachedUserLexemeEntity(
        id = id,
        userId = userId,
        language = language,
        canonicalText = canonicalText,
        canonicalKey = canonicalKey,
        partOfSpeech = partOfSpeech,
        updatedAt = updatedAt,
    )

private fun CardSnapshot.toEntity(updatedAt: String): CachedCardSnapshotEntity =
    CachedCardSnapshotEntity(
        id = id,
        userId = userId,
        lexemeId = lexemeId,
        schemaVersion = schemaVersion,
        provider = provider,
        model = model,
        resultLanguage = resultLanguage,
        contentJson = vocabularyJson.encodeToString(content),
        active = active,
        createdAt = createdAt,
        updatedAt = updatedAt,
    )

private fun LexemeForm.toEntity(updatedAt: String): CachedLexemeFormEntity =
    CachedLexemeFormEntity(
        id = id,
        userId = userId,
        lexemeId = lexemeId,
        language = language,
        formText = formText,
        formKey = formKey,
        relation = relation,
        source = source,
        updatedAt = updatedAt,
    )
