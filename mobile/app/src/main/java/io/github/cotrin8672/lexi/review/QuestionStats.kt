package io.github.cotrin8672.lexi.review

data class QuestionStats(
    val questionKey: String,
    val questionType: QuestionType,
    val lexemeId: String,
    val attempts: Int = 0,
    val correctCount: Int = 0,
    val wrongCount: Int = 0,
    val correctStreak: Int = 0,
    val wrongStreak: Int = 0,
    val difficultyEma: Double = 0.5,
    val lastResult: ReviewResult? = null,
    val lastReviewedAt: String? = null,
    val lastSeenSequence: Long? = null,
    val createdAt: String,
    val updatedAt: String,
)

enum class ReviewResult {
    CORRECT,
    WRONG,
}
