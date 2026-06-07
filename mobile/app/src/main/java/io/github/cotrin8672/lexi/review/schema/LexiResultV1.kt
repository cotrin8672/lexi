package io.github.cotrin8672.lexi.review.schema

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

const val LEXI_RESULT_V1_SCHEMA_VERSION = "lexi.result.v1"

@Serializable
data class ExampleSentence(
    val sentence: String,
    val japanese: String,
)

@Serializable
data class Translation(
    val text: String,
    val note: String? = null,
    val example: ExampleSentence,
    val senseKind: String? = null,
    val baseWord: String? = null,
)

@Serializable
data class RelatedWord(
    val term: String,
    val japanese: String,
    val usageComparison: String,
)

@Serializable
data class Idiom(
    val idiom: String,
    val japanese: String,
    val example: String,
)

@Serializable
data class Inflection(
    val kind: String,
    val form: String,
)

@Serializable
data class LexiResultV1(
    @SerialName("schemaVersion") val schemaVersion: String,
    val mode: String,
    val sourceLanguage: String,
    val resultLanguage: String,
    val headword: String,
    val inflections: List<Inflection> = emptyList(),
    val translations: List<Translation> = emptyList(),
    val nuance: String,
    val synonyms: List<RelatedWord> = emptyList(),
    val idioms: List<Idiom> = emptyList(),
    val warnings: List<String> = emptyList(),
)
