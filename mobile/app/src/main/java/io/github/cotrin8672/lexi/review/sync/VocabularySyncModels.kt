package io.github.cotrin8672.lexi.review.sync

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class BootstrapLexemeRow(
    val id: String,
    val language: String,
    @SerialName("canonical_text") val canonicalText: String,
    @SerialName("canonical_key") val canonicalKey: String,
    @SerialName("part_of_speech") val partOfSpeech: String? = null,
    @SerialName("created_at") val createdAt: String? = null,
)

@Serializable
data class BootstrapFormRow(
    val id: String,
    @SerialName("lexeme_id") val lexemeId: String,
    val language: String,
    @SerialName("form_text") val formText: String,
    @SerialName("form_key") val formKey: String,
    val relation: String,
    val source: String,
)

@Serializable
data class BootstrapCardRow(
    val id: String,
    @SerialName("lexeme_id") val lexemeId: String,
    @SerialName("schema_version") val schemaVersion: String,
    val provider: String? = null,
    val model: String? = null,
    @SerialName("result_language") val resultLanguage: String,
    val content: JsonElement,
    val active: Boolean,
    @SerialName("created_at") val createdAt: String? = null,
)

@Serializable
data class MaxRevisionRow(
    @SerialName("server_revision") val serverRevision: Long,
)

@Serializable
data class PulledChange(
    @SerialName("serverRevision") val serverRevision: Long,
    @SerialName("operationId") val operationId: String,
    @SerialName("entityType") val entityType: String,
    @SerialName("entityId") val entityId: String? = null,
    @SerialName("changeType") val changeType: String,
    val payload: JsonElement,
)

@Serializable
data class PullResponse(
    val changes: List<PulledChange> = emptyList(),
    @SerialName("lastRevision") val lastRevision: Long,
)
