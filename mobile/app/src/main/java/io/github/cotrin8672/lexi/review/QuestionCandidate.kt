package io.github.cotrin8672.lexi.review

data class QuestionCandidate(
    val questionKey: String,
    val questionType: QuestionType,
    val lexemeId: String,
    val sourceHash: String,
    val renderSeedHint: String,
    val payload: QuestionPayload,
)

sealed interface QuestionPayload {
    data class Meaning(
        val headword: String,
        val correctMeaning: String,
        val partOfSpeechNote: String?,
    ) : QuestionPayload

    data class Reorder(
        val promptJapanese: String,
        val originalSentence: String,
        val tokens: List<String>,
    ) : QuestionPayload

    data class Usage(
        val prompt: String,
        val correctTerm: String,
        val headword: String,
        val otherTerm: String,
        val comparison: String,
    ) : QuestionPayload

    data class Inflection(
        val headword: String,
        val formText: String,
        val relation: String,
        val formKey: String,
        val direction: InflectionDirection,
    ) : QuestionPayload
}

enum class InflectionDirection {
    BASE_TO_FORM,
    FORM_TO_BASE,
    KIND_RECOGNITION,
}
