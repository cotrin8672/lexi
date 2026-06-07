package io.github.cotrin8672.lexi.review

import kotlin.random.Random

data class WeightingContext(
    val recentQuestionKeys: List<String> = emptyList(),
    val recentLexemeIds: List<String> = emptyList(),
)

data class WeightedCandidate(
    val candidate: QuestionCandidate,
    val weight: Double,
)

private val TYPE_BALANCE = mapOf(
    QuestionType.MEANING to 0.40,
    QuestionType.REORDER to 0.30,
    QuestionType.USAGE to 0.20,
    QuestionType.INFLECTION to 0.10,
)

fun computeWeight(
    candidate: QuestionCandidate,
    stats: QuestionStats?,
    context: WeightingContext,
): Double {
    val base = 1.0
    val typeBalance = TYPE_BALANCE[candidate.questionType] ?: 1.0
    val difficultyEma = stats?.difficultyEma ?: 0.5
    val weakness = 1.0 + 6.0 * difficultyEma
    val recentMistakeBoost = if (stats?.lastResult == ReviewResult.WRONG) 2.5 else 1.0
    val masteryPenalty = when (stats?.correctStreak ?: 0) {
        in 5..Int.MAX_VALUE -> 0.15
        4 -> 0.25
        3 -> 0.45
        2 -> 0.70
        else -> 1.0
    }
    val freshnessPenalty = when {
        candidate.questionKey in context.recentQuestionKeys.take(3) -> 0.05
        candidate.questionKey in context.recentQuestionKeys.take(10) -> 0.30
        else -> 1.0
    }
    val lexemeFreshnessPenalty = if (candidate.lexemeId in context.recentLexemeIds.take(3)) {
        0.25
    } else {
        1.0
    }
    return base * typeBalance * weakness * recentMistakeBoost *
        masteryPenalty * freshnessPenalty * lexemeFreshnessPenalty
}

fun sampleWeighted(
    candidates: List<QuestionCandidate>,
    statsByKey: Map<String, QuestionStats>,
    context: WeightingContext,
    random: Random = Random.Default,
): QuestionCandidate? {
    if (candidates.isEmpty()) {
        return null
    }
    val weighted = candidates.map { candidate ->
        WeightedCandidate(
            candidate = candidate,
            weight = computeWeight(candidate, statsByKey[candidate.questionKey], context),
        )
    }
    val total = weighted.sumOf { it.weight }
    if (total <= 0.0) {
        return candidates.random(random)
    }
    var draw = random.nextDouble(total)
    for (entry in weighted) {
        draw -= entry.weight
        if (draw <= 0.0) {
            return entry.candidate
        }
    }
    return weighted.last().candidate
}
