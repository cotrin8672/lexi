package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.schema.CardSnapshot
import io.github.cotrin8672.lexi.review.schema.LexemeForm
import io.github.cotrin8672.lexi.review.schema.LexiResultV1
import io.github.cotrin8672.lexi.review.schema.UserLexeme
import io.github.cotrin8672.lexi.review.schema.VocabularyBundle
import io.github.cotrin8672.lexi.review.sync.SupabaseMobileConfig
import io.github.cotrin8672.lexi.review.sync.SupabaseSession
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.decodeFromJsonElement

class SupabasePostgrestVocabularyClient(
    private val config: SupabaseMobileConfig,
    private val sessionProvider: suspend () -> SupabaseSession?,
) : SupabaseVocabularyClient {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    override suspend fun fetchActiveVocabulary(userId: String): VocabularyBundle {
        val session = sessionProvider()
            ?: error("Supabase session is not available")
        val lexemes = get<List<UserLexemeRow>>(
            path = "rest/v1/user_lexemes",
            query = "select=id,user_id,language,canonical_text,canonical_key,part_of_speech&deleted_at=is.null",
            accessToken = session.accessToken,
        ).map { it.toDomain() }
        val snapshots = get<List<CardSnapshotRow>>(
            path = "rest/v1/card_snapshots",
            query = "select=id,user_id,lexeme_id,schema_version,provider,model,result_language,content,active,created_at&active=eq.true",
            accessToken = session.accessToken,
        ).map { it.toDomain(json) }
        val forms = get<List<LexemeFormRow>>(
            path = "rest/v1/lexeme_forms",
            query = "select=id,user_id,lexeme_id,language,form_text,form_key,relation,source",
            accessToken = session.accessToken,
        ).map { it.toDomain() }

        return VocabularyBundle(
            lexemes = lexemes.filter { it.userId == userId },
            snapshots = snapshots.filter { it.userId == userId },
            forms = forms.filter { it.userId == userId },
        )
    }

    private inline fun <reified T> get(
        path: String,
        query: String,
        accessToken: String,
    ): T {
        val base = config.url.trimEnd('/')
        val connection = URL("$base/$path?$query").openConnection() as HttpURLConnection
        connection.requestMethod = "GET"
        connection.setRequestProperty("apikey", config.publishableKey)
        connection.setRequestProperty("Authorization", "Bearer $accessToken")
        connection.setRequestProperty("Accept", "application/json")
        connection.connectTimeout = 15_000
        connection.readTimeout = 20_000
        val status = connection.responseCode
        val body = if (status in 200..299) {
            connection.inputStream.bufferedReader().use { it.readText() }
        } else {
            connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
        }
        if (status !in 200..299) {
            error("Supabase vocabulary fetch failed with HTTP $status")
        }
        return json.decodeFromString(body)
    }
}

@Serializable
private data class UserLexemeRow(
    val id: String,
    @SerialName("user_id") val userId: String,
    val language: String,
    @SerialName("canonical_text") val canonicalText: String,
    @SerialName("canonical_key") val canonicalKey: String,
    @SerialName("part_of_speech") val partOfSpeech: String? = null,
) {
    fun toDomain(): UserLexeme = UserLexeme(
        id = id,
        userId = userId,
        language = language,
        canonicalText = canonicalText,
        canonicalKey = canonicalKey,
        partOfSpeech = partOfSpeech,
    )
}

@Serializable
private data class CardSnapshotRow(
    val id: String,
    @SerialName("user_id") val userId: String,
    @SerialName("lexeme_id") val lexemeId: String,
    @SerialName("schema_version") val schemaVersion: String,
    val provider: String? = null,
    val model: String? = null,
    @SerialName("result_language") val resultLanguage: String,
    val content: JsonElement,
    val active: Boolean,
    @SerialName("created_at") val createdAt: String,
) {
    fun toDomain(json: Json): CardSnapshot = CardSnapshot(
        id = id,
        userId = userId,
        lexemeId = lexemeId,
        schemaVersion = schemaVersion,
        provider = provider ?: "supabase",
        model = model ?: "unknown",
        resultLanguage = resultLanguage,
        content = json.decodeFromJsonElement<LexiResultV1>(content),
        active = active,
        createdAt = createdAt,
    )
}

@Serializable
private data class LexemeFormRow(
    val id: String,
    @SerialName("user_id") val userId: String,
    @SerialName("lexeme_id") val lexemeId: String,
    val language: String,
    @SerialName("form_text") val formText: String,
    @SerialName("form_key") val formKey: String,
    val relation: String,
    val source: String,
) {
    fun toDomain(): LexemeForm = LexemeForm(
        id = id,
        userId = userId,
        lexemeId = lexemeId,
        language = language,
        formText = formText,
        formKey = formKey,
        relation = relation,
        source = source,
    )
}
