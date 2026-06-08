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
import java.util.UUID
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive

internal object VocabularyPull {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    suspend fun run(
        restClient: SupabaseRestClient,
        cacheDao: VocabularyCacheDao,
        syncStateDao: VocabularySyncStateDao,
        userId: String,
        accessToken: String,
    ) {
        var sinceRevision = syncStateDao.get(userId)?.lastServerRevision ?: 0L
        while (true) {
            val pull = restClient.pullVocabularyChanges(
                accessToken = accessToken,
                sinceRevision = sinceRevision,
            )
            validatePullResponse(pull, sinceRevision)

            if (pull.changes.isEmpty()) {
                if (pull.lastRevision > sinceRevision) {
                    updateRevision(syncStateDao, userId, pull.lastRevision)
                }
                break
            }

            for (change in pull.changes) {
                applyChange(cacheDao, userId, change)
                sinceRevision = change.serverRevision
            }
            updateRevision(syncStateDao, userId, sinceRevision)

            if (pull.changes.size < SupabaseRestClient.PULL_BATCH_LIMIT) {
                break
            }
        }
    }

    private fun validatePullResponse(pull: PullResponse, sinceRevision: Long) {
        val maxChangeRevision = pull.changes.maxOfOrNull { it.serverRevision } ?: 0L
        if (pull.lastRevision < maxChangeRevision) {
            error(
                "Supabase pull lastRevision ${pull.lastRevision} is behind max change revision $maxChangeRevision",
            )
        }
        if (pull.lastRevision < sinceRevision) {
            error(
                "Supabase pull lastRevision ${pull.lastRevision} moved backwards from $sinceRevision",
            )
        }
    }

    private suspend fun updateRevision(
        syncStateDao: VocabularySyncStateDao,
        userId: String,
        revision: Long,
    ) {
        val now = Instant.now().toString()
        val existing = syncStateDao.get(userId)
        syncStateDao.upsert(
            VocabularySyncStateEntity(
                userId = userId,
                bootstrapComplete = existing?.bootstrapComplete ?: true,
                lastServerRevision = revision,
                updatedAt = now,
            ),
        )
    }

    private suspend fun applyChange(
        cacheDao: VocabularyCacheDao,
        userId: String,
        change: PulledChange,
    ) {
        if (change.entityType != "card_snapshot" || change.changeType != "upsert") {
            return
        }
        if (cacheDao.hasAppliedPullChange(userId, change.operationId, change.serverRevision)) {
            return
        }

        val payload = change.payload as? JsonObject
            ?: error("Supabase pull payload was not an object")
        val language = payload.stringValue("language") ?: "en"
        val canonicalText = payload.requiredString("canonicalText")
        val canonicalKey = payload.requiredString("canonicalKey")
        val resultLanguage = payload.requiredString("resultLanguage")
        val schemaVersion = payload.requiredString("schemaVersion")
        val provider = payload.stringValue("provider") ?: "supabase"
        val model = payload.stringValue("model") ?: "unknown"
        val content = payload["content"]
            ?: error("Supabase pull payload missing content")
        if (schemaVersion != LEXI_RESULT_V1_SCHEMA_VERSION) {
            return
        }
        val parsedContent = json.decodeFromJsonElement<LexiResultV1>(content)
        if (parsedContent.mode != "word-study") {
            return
        }

        val now = Instant.now().toString()
        val lexemeId = payload.stringValue("lexemeId")
            ?: cacheDao.findLexemeId(userId, language, canonicalKey)
            ?: UUID.randomUUID().toString()

        val partOfSpeech = parsedContent.translations.firstOrNull()?.note
        cacheDao.upsertLexeme(
            CachedUserLexemeEntity(
                id = lexemeId,
                userId = userId,
                language = language,
                canonicalText = canonicalText,
                canonicalKey = canonicalKey,
                partOfSpeech = partOfSpeech,
                updatedAt = now,
            ),
        )

        payload["forms"]?.jsonArray?.forEach { formElement ->
            val form = formElement as? JsonObject ?: return@forEach
            val formText = form.stringValue("formText") ?: return@forEach
            val relation = form.stringValue("relation") ?: "observed"
            val source = form.stringValue("source") ?: "sync"
            val formKey = form.stringValue("formKey") ?: formText.lowercase()
            cacheDao.upsertForm(
                CachedLexemeFormEntity(
                    id = form.stringValue("id") ?: UUID.randomUUID().toString(),
                    userId = userId,
                    lexemeId = lexemeId,
                    language = language,
                    formText = formText,
                    formKey = formKey,
                    relation = relation,
                    source = source,
                    updatedAt = now,
                ),
            )
        }

        cacheDao.deactivateSnapshots(userId, lexemeId, resultLanguage)
        cacheDao.upsertSnapshot(
            CachedCardSnapshotEntity(
                id = payload.stringValue("cardSnapshotId") ?: UUID.randomUUID().toString(),
                userId = userId,
                lexemeId = lexemeId,
                schemaVersion = schemaVersion,
                provider = provider,
                model = model,
                resultLanguage = resultLanguage,
                contentJson = json.encodeToString(LexiResultV1.serializer(), parsedContent),
                active = true,
                createdAt = now,
                updatedAt = now,
                remoteOperationId = change.operationId,
                remoteServerRevision = change.serverRevision,
            ),
        )
    }

    private fun JsonObject.stringValue(key: String): String? =
        (this[key] as? JsonPrimitive)?.content

    private fun JsonObject.requiredString(key: String): String =
        stringValue(key) ?: error("Supabase pull payload missing $key")
}
