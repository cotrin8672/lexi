package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import kotlin.random.Random

data class MeaningOption(
    val answerKey: String,
    val label: String,
    val isCorrect: Boolean,
)

data class MeaningOptions(
    val questionKey: String,
    val options: List<MeaningOption>,
)

data class ReorderPresentation(
    val questionKey: String,
    val shuffledTokens: List<String>,
)

fun generateMeaningOptions(
    candidate: QuestionCandidate,
    cards: List<ActiveVocabularyCard>,
    random: Random = Random.Default,
): MeaningOptions? {
    val payload = candidate.payload as? QuestionPayload.Meaning ?: return null
    val meaningSenses = cards.flatMap { card ->
        card.content.translations.map { translation ->
            MeaningSense(
                lexemeId = card.lexemeId,
                meaning = translation.text,
                partOfSpeechNote = translation.note,
            )
        }
    }
    val normalizedCorrect = normalizeJapaneseMeaning(payload.correctMeaning)
    val samePos = meaningSenses.filter {
        it.lexemeId != candidate.lexemeId &&
            payload.partOfSpeechNote != null &&
            it.partOfSpeechNote == payload.partOfSpeechNote &&
            normalizeJapaneseMeaning(it.meaning) != normalizedCorrect
    }
    val pool = (if (samePos.size >= 3) samePos else {
        meaningSenses.filter {
            it.lexemeId != candidate.lexemeId &&
                normalizeJapaneseMeaning(it.meaning) != normalizedCorrect
        }
    }).map { normalizeJapaneseMeaning(it.meaning) to it.meaning }
        .distinctBy { it.first }

    if (pool.size < 3) {
        return null
    }
    val distractors = pool.shuffled(random).take(3).map { (_, label) ->
        MeaningOption(
            answerKey = hashForQuestionKey(normalizeJapaneseMeaning(label)),
            label = label,
            isCorrect = false,
        )
    }
    val correct = MeaningOption(
        answerKey = hashForQuestionKey(normalizedCorrect),
        label = payload.correctMeaning,
        isCorrect = true,
    )
    return MeaningOptions(
        questionKey = candidate.questionKey,
        options = (distractors + correct).shuffled(random),
    )
}

fun generateReorderPresentation(
    candidate: QuestionCandidate,
    random: Random = Random.Default,
    maxShuffleAttempts: Int = 12,
): ReorderPresentation? {
    val payload = candidate.payload as? QuestionPayload.Reorder ?: return null
    val original = payload.tokens
    if (original.size < 2) {
        return null
    }
    repeat(maxShuffleAttempts) {
        val shuffled = original.shuffled(random)
        if (shuffled != original) {
            return ReorderPresentation(
                questionKey = candidate.questionKey,
                shuffledTokens = shuffled,
            )
        }
    }
    return null
}

fun isReorderAnswerCorrect(originalSentence: String, submittedTokens: List<String>): Boolean {
    val expected = tokenizeEnglishSentence(originalSentence) ?: return false
    return submittedTokens.map { normalizeEnglish(it) } == expected.map { normalizeEnglish(it) }
}

fun inflectionPrompt(payload: QuestionPayload.Inflection): String =
    when (payload.direction) {
        InflectionDirection.BASE_TO_FORM ->
            "What is the ${payload.relation} form of \"${payload.headword}\"?"
        InflectionDirection.FORM_TO_BASE ->
            "What is the base form of \"${payload.formText}\"?"
        InflectionDirection.KIND_RECOGNITION ->
            "Which form is \"${payload.formText}\"?"
    }

fun isInflectionAnswerCorrect(
    payload: QuestionPayload.Inflection,
    submittedAnswer: String,
): Boolean =
    normalizeEnglish(submittedAnswer) == normalizeEnglish(payload.formText)

fun generateUsageOptions(
    candidate: QuestionCandidate,
    cards: List<ActiveVocabularyCard>,
    random: Random = Random.Default,
): List<MeaningOption>? {
    val payload = candidate.payload as? QuestionPayload.Usage ?: return null
    val card = cards.firstOrNull { it.lexemeId == candidate.lexemeId } ?: return null
    val nearTerms = buildList {
        add(card.content.headword)
        addAll(card.content.synonyms.map { it.term })
    }.map { normalizeEnglish(it) to it }
        .distinctBy { it.first }
        .map { it.second }

    if (nearTerms.size < 2) {
        return null
    }
    val distractors = nearTerms
        .filter { normalizeEnglish(it) != normalizeEnglish(payload.correctTerm) }
        .shuffled(random)
        .take(3)
        .map { term ->
            MeaningOption(
                answerKey = normalizeEnglish(term),
                label = term,
                isCorrect = false,
            )
        }
    if (distractors.size < 3) {
        return null
    }
    val correct = MeaningOption(
        answerKey = normalizeEnglish(payload.correctTerm),
        label = payload.correctTerm,
        isCorrect = true,
    )
    return (distractors + correct).shuffled(random)
}
