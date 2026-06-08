package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.schema.LexemeForm

data class MeaningSense(
    val lexemeId: String,
    val meaning: String,
    val partOfSpeechNote: String?,
)

fun extractQuestionCandidates(cards: List<ActiveVocabularyCard>): List<QuestionCandidate> {
    if (cards.isEmpty()) {
        return emptyList()
    }
    val meaningSenses = collectMeaningSenses(cards)
    val candidates = mutableListOf<QuestionCandidate>()
    candidates += extractMeaningCandidates(cards, meaningSenses)
    candidates += extractReorderCandidates(cards)
    candidates += extractUsageCandidates(cards)
    candidates += extractInflectionCandidates(cards)
    return candidates
}

private fun collectMeaningSenses(cards: List<ActiveVocabularyCard>): List<MeaningSense> =
    cards.flatMap { card ->
        card.content.translations.map { translation ->
            MeaningSense(
                lexemeId = card.lexemeId,
                meaning = translation.text,
                partOfSpeechNote = translation.note,
            )
        }
    }

private fun extractMeaningCandidates(
    cards: List<ActiveVocabularyCard>,
    meaningSenses: List<MeaningSense>,
): List<QuestionCandidate> {
    val candidates = mutableListOf<QuestionCandidate>()
    for (card in cards) {
        for (translation in card.content.translations) {
            val correctMeaning = translation.text
            val distractorCount = countMeaningDistractors(
                correctLexemeId = card.lexemeId,
                correctMeaning = correctMeaning,
                preferredNote = translation.note,
                meaningSenses = meaningSenses,
            )
            if (distractorCount < 3) {
                continue
            }
            val key = QuestionKey.meaning(card.lexemeId, correctMeaning)
            candidates += QuestionCandidate(
                questionKey = key,
                questionType = QuestionType.MEANING,
                lexemeId = card.lexemeId,
                sourceHash = hashForQuestionKey(normalizeJapaneseMeaning(correctMeaning)),
                renderSeedHint = key,
                payload = QuestionPayload.Meaning(
                    headword = card.content.headword,
                    correctMeaning = correctMeaning,
                    partOfSpeechNote = translation.note,
                ),
            )
        }
    }
    return candidates
}

fun countMeaningDistractors(
    correctLexemeId: String,
    correctMeaning: String,
    preferredNote: String?,
    meaningSenses: List<MeaningSense>,
): Int {
    val normalizedCorrect = normalizeJapaneseMeaning(correctMeaning)
    val samePos = meaningSenses.filter {
        it.lexemeId != correctLexemeId &&
            preferredNote != null &&
            it.partOfSpeechNote == preferredNote &&
            normalizeJapaneseMeaning(it.meaning) != normalizedCorrect
    }
    val pool = if (samePos.size >= 3) {
        samePos
    } else {
        meaningSenses.filter {
            it.lexemeId != correctLexemeId &&
                normalizeJapaneseMeaning(it.meaning) != normalizedCorrect
        }
    }
    return pool.map { normalizeJapaneseMeaning(it.meaning) }.distinct().size
}

private fun extractReorderCandidates(cards: List<ActiveVocabularyCard>): List<QuestionCandidate> {
    val candidates = mutableListOf<QuestionCandidate>()
    for (card in cards) {
        for (translation in card.content.translations) {
            val sentence = translation.example.sentence
            val tokens = tokenizeEnglishSentence(sentence) ?: continue
            val key = QuestionKey.reorder(card.lexemeId, sentence)
            candidates += QuestionCandidate(
                questionKey = key,
                questionType = QuestionType.REORDER,
                lexemeId = card.lexemeId,
                sourceHash = hashForQuestionKey(normalizeEnglish(sentence)),
                renderSeedHint = key,
                payload = QuestionPayload.Reorder(
                    promptJapanese = translation.example.japanese,
                    originalSentence = sentence,
                    tokens = tokens,
                ),
            )
        }
    }
    return candidates
}

private fun extractUsageCandidates(cards: List<ActiveVocabularyCard>): List<QuestionCandidate> {
    val candidates = mutableListOf<QuestionCandidate>()
    for (card in cards) {
        val headword = card.content.headword
        for (synonym in card.content.synonyms) {
            if (!isUsageComparisonConcrete(synonym.usageComparison, headword, synonym.term)) {
                continue
            }
            val key = QuestionKey.usage(
                lexemeId = card.lexemeId,
                headword = headword,
                otherTerm = synonym.term,
                comparison = synonym.usageComparison,
            )
            candidates += QuestionCandidate(
                questionKey = key,
                questionType = QuestionType.USAGE,
                lexemeId = card.lexemeId,
                sourceHash = hashForQuestionKey(
                    listOf(headword, synonym.term, synonym.usageComparison)
                        .joinToString("|") { normalizeEnglish(it) },
                ),
                renderSeedHint = key,
                payload = QuestionPayload.Usage(
                    prompt = synonym.usageComparison,
                    correctTerm = synonym.term,
                    headword = headword,
                    otherTerm = synonym.term,
                    comparison = synonym.usageComparison,
                ),
            )
        }
    }
    return candidates
}

private fun extractInflectionCandidates(cards: List<ActiveVocabularyCard>): List<QuestionCandidate> {
    val candidates = mutableListOf<QuestionCandidate>()
    for (card in cards) {
        val headword = card.content.headword
        val inflectionPoints = mutableListOf<InflectionPoint>()

        for (inflection in card.content.inflections) {
            if (normalizeEnglish(inflection.form) == normalizeEnglish(headword)) {
                continue
            }
            inflectionPoints += InflectionPoint(
                formText = inflection.form,
                relation = inflection.kind,
                formKey = normalizeLookupKey(inflection.form),
                priority = 0,
            )
        }

        for (form in card.forms) {
            if (form.relation != IRREGULAR_RELATION) {
                continue
            }
            if (normalizeEnglish(form.formText) == normalizeEnglish(headword)) {
                continue
            }
            inflectionPoints += InflectionPoint(
                formText = form.formText,
                relation = form.relation,
                formKey = form.formKey,
                priority = 0,
            )
        }

        for (point in inflectionPoints
            .groupBy { it.formKey }
            .values
            .map { points -> points.minBy { inflectionRelationPriority(it.relation) } }
        ) {
            val key = QuestionKey.inflection(card.lexemeId, point.relation, point.formKey)
            candidates += QuestionCandidate(
                questionKey = key,
                questionType = QuestionType.INFLECTION,
                lexemeId = card.lexemeId,
                sourceHash = hashForQuestionKey("${point.relation}|${point.formKey}"),
                renderSeedHint = key,
                payload = QuestionPayload.Inflection(
                    headword = headword,
                    formText = point.formText,
                    relation = point.relation,
                    formKey = point.formKey,
                    direction = InflectionDirection.BASE_TO_FORM,
                ),
            )
        }
    }
    return candidates
}

private const val IRREGULAR_RELATION = "irregular"

private data class InflectionPoint(
    val formText: String,
    val relation: String,
    val formKey: String,
    val priority: Int,
)

private fun inflectionRelationPriority(relation: String): Int =
    when (relation.lowercase()) {
        "past", "pastparticiple", "past_participle", "plural" -> 0
        "irregular" -> 1
        else -> 2
    }

fun shouldSkipInflectionForm(headword: String, formText: String): Boolean =
    normalizeEnglish(formText) == normalizeEnglish(headword)

fun shouldSkipReorderSentence(sentence: String): Boolean =
    tokenizeEnglishSentence(sentence) == null

fun shouldSkipUsageComparison(
    comparison: String,
    headword: String,
    otherTerm: String,
): Boolean = !isUsageComparisonConcrete(comparison, headword, otherTerm)
