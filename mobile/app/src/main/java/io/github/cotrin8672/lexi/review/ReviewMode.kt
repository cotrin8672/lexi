package io.github.cotrin8672.lexi.review

enum class ReviewMode(val label: String) {
    MEANING_ONLY("Meaning only"),
    REORDER_ONLY("Sentence reorder only"),
    USAGE_ONLY("Usage only"),
    INFLECTION_ONLY("Inflection only"),
    MIXED_RANDOM("Mixed random"),
}

fun List<QuestionCandidate>.filterForMode(mode: ReviewMode): List<QuestionCandidate> = when (mode) {
    ReviewMode.MIXED_RANDOM -> this
    ReviewMode.MEANING_ONLY -> filter { it.questionType == QuestionType.MEANING }
    ReviewMode.REORDER_ONLY -> filter { it.questionType == QuestionType.REORDER }
    ReviewMode.USAGE_ONLY -> filter { it.questionType == QuestionType.USAGE }
    ReviewMode.INFLECTION_ONLY -> filter { it.questionType == QuestionType.INFLECTION }
}
