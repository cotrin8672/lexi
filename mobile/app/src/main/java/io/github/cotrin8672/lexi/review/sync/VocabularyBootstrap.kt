package io.github.cotrin8672.lexi.review.sync

import io.github.cotrin8672.lexi.review.schema.LEXI_RESULT_V1_SCHEMA_VERSION
import io.github.cotrin8672.lexi.review.schema.LexiResultV1
import io.github.cotrin8672.lexi.review.storage.dao.VocabularyCacheDao
import io.github.cotrin8672.lexi.review.storage.dao.VocabularySyncStateDao
import io.github.cotrin8672.lexi.review.storage.entity.CachedCardSnapshotEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedLexemeFormEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedUserLexemeEntity
import io.github.cotrin8672.lexi.review.storage.entity.VocabularySyncStateEntity
import java.time.Instant
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.decodeFromJsonElement

internal object VocabularyBootstrap {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    suspend fun runIfNeeded(
        restClient: SupabaseRestClient,
        cacheDao: VocabularyCacheDao,
        syncStateDao: VocabularySyncStateDao,
        userId: String,
        accessToken: String,
    ) {
        val existing = syncStateDao.get(userId)
        if (existing?.bootstrapComplete == true) {
            return
        }

        val now = Instant.now().toString()
        val baseRevision = runCatching {
            restClient.fetchMaxServerRevision(accessToken)
        }.getOrDefault(0L)

        val lexemes = restClient.getAllRows<BootstrapLexemeRow>(
            accessToken = accessToken,
            table = "user_lexemes",
            select = "id,language,canonical_text,canonical_key,part_of_speech,created_at",
            filters = listOf("deleted_at" to "is.null"),
        )
        val forms = restClient.getAllRows<BootstrapFormRow>(
            accessToken = accessToken,
            table = "lexeme_forms",
            select = "id,lexeme_id,language,form_text,form_key,relation,source",
        )
        val cards = restClient.getAllRows<BootstrapCardRow>(
            accessToken = accessToken,
            table = "card_snapshots",
            select = "id,lexeme_id,schema_version,provider,model,result_language,content,active,created_at",
            filters = listOf("active" to "eq.true"),
        )

        val lexemeEntities = lexemes
            .filter { it.id.isNotBlank() }
            .map {
                CachedUserLexemeEntity(
                    id = it.id,
                    userId = userId,
                    language = it.language,
                    canonicalText = it.canonicalText,
                    canonicalKey = it.canonicalKey,
                    partOfSpeech = it.partOfSpeech,
                    createdAt = bootstrapLexemeCreatedAt(it.createdAt, now),
                    updatedAt = now,
                )
            }
        val formEntities = forms
            .filter { it.id.isNotBlank() }
            .map {
                CachedLexemeFormEntity(
                    id = it.id,
                    userId = userId,
                    lexemeId = it.lexemeId,
                    language = it.language,
                    formText = it.formText,
                    formKey = it.formKey,
                    relation = it.relation,
                    source = it.source,
                    updatedAt = now,
                )
            }
        val snapshotEntities = cards.mapNotNull { row ->
            toCachedSnapshotOrNull(userId, row, now)
        }

        cacheDao.replaceUserCache(
            userId = userId,
            lexemes = lexemeEntities,
            snapshots = snapshotEntities,
            forms = formEntities,
        )
        syncStateDao.upsert(
            VocabularySyncStateEntity(
                userId = userId,
                bootstrapComplete = true,
                lastServerRevision = baseRevision,
                updatedAt = now,
            ),
        )
    }

    private fun toCachedSnapshotOrNull(
        userId: String,
        row: BootstrapCardRow,
        updatedAt: String,
    ): CachedCardSnapshotEntity? = runCatching {
        if (!row.active || row.schemaVersion != LEXI_RESULT_V1_SCHEMA_VERSION) {
            return@runCatching null
        }
        val content = json.decodeFromJsonElement<LexiResultV1>(row.content)
        if (content.mode != "word-study") {
            return@runCatching null
        }
        CachedCardSnapshotEntity(
            id = row.id,
            userId = userId,
            lexemeId = row.lexemeId,
            schemaVersion = row.schemaVersion,
            provider = row.provider ?: "supabase",
            model = row.model ?: "unknown",
            resultLanguage = row.resultLanguage,
            contentJson = json.encodeToString(LexiResultV1.serializer(), content),
            active = true,
            createdAt = row.createdAt ?: updatedAt,
            updatedAt = updatedAt,
        )
    }.getOrNull()
}
