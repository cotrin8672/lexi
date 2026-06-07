package io.github.cotrin8672.lexi.review

const val DIFFICULTY_EMA_ALPHA = 0.25

fun applyAnswer(
    existing: QuestionStats?,
    candidate: QuestionCandidate,
    correct: Boolean,
    reviewedAt: String,
    seenSequence: Long,
): QuestionStats {
    val now = reviewedAt
    val outcomeWrong = if (correct) 0.0 else 1.0
    val base = existing ?: QuestionStats(
        questionKey = candidate.questionKey,
        questionType = candidate.questionType,
        lexemeId = candidate.lexemeId,
        createdAt = now,
        updatedAt = now,
    )
    val nextDifficulty = if (base.attempts == 0) {
        0.5 * (1 - DIFFICULTY_EMA_ALPHA) + outcomeWrong * DIFFICULTY_EMA_ALPHA
    } else {
        base.difficultyEma * (1 - DIFFICULTY_EMA_ALPHA) + outcomeWrong * DIFFICULTY_EMA_ALPHA
    }
    return if (correct) {
        base.copy(
            attempts = base.attempts + 1,
            correctCount = base.correctCount + 1,
            correctStreak = base.correctStreak + 1,
            wrongStreak = 0,
            difficultyEma = nextDifficulty,
            lastResult = ReviewResult.CORRECT,
            lastReviewedAt = now,
            lastSeenSequence = seenSequence,
            updatedAt = now,
        )
    } else {
        base.copy(
            attempts = base.attempts + 1,
            wrongCount = base.wrongCount + 1,
            wrongStreak = base.wrongStreak + 1,
            correctStreak = 0,
            difficultyEma = nextDifficulty,
            lastResult = ReviewResult.WRONG,
            lastReviewedAt = now,
            lastSeenSequence = seenSequence,
            updatedAt = now,
        )
    }
}
