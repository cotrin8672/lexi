package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.QuestionStats
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.ReviewResult
import io.github.cotrin8672.lexi.review.storage.dao.QuestionStatsDao
import io.github.cotrin8672.lexi.review.storage.entity.QuestionStatsEntity

interface ReviewStore {
    suspend fun getStats(questionKey: String): QuestionStats?
    suspend fun getAllStats(): List<QuestionStats>
    suspend fun getStatsByKeys(questionKeys: Collection<String>): Map<String, QuestionStats>
    suspend fun upsertStats(stats: QuestionStats)
}

class RoomReviewStore(
    private val dao: QuestionStatsDao,
) : ReviewStore {
    override suspend fun getStats(questionKey: String): QuestionStats? =
        dao.getByKey(questionKey)?.toDomain()

    override suspend fun getAllStats(): List<QuestionStats> =
        dao.getAll().map { it.toDomain() }

    override suspend fun getStatsByKeys(questionKeys: Collection<String>): Map<String, QuestionStats> {
        if (questionKeys.isEmpty()) {
            return emptyMap()
        }
        return dao.getByKeys(questionKeys.toList())
            .associate { it.questionKey to it.toDomain() }
    }

    override suspend fun upsertStats(stats: QuestionStats) {
        dao.upsert(stats.toEntity())
    }
}

private fun QuestionStatsEntity.toDomain(): QuestionStats = QuestionStats(
    questionKey = questionKey,
    questionType = QuestionType.valueOf(questionType),
    lexemeId = lexemeId,
    attempts = attempts,
    correctCount = correctCount,
    wrongCount = wrongCount,
    correctStreak = correctStreak,
    wrongStreak = wrongStreak,
    difficultyEma = difficultyEma,
    lastResult = lastResult?.let { ReviewResult.valueOf(it) },
    lastReviewedAt = lastReviewedAt,
    lastSeenSequence = lastSeenSequence,
    createdAt = createdAt,
    updatedAt = updatedAt,
)

private fun QuestionStats.toEntity(): QuestionStatsEntity = QuestionStatsEntity(
    questionKey = questionKey,
    questionType = questionType.name,
    lexemeId = lexemeId,
    attempts = attempts,
    correctCount = correctCount,
    wrongCount = wrongCount,
    correctStreak = correctStreak,
    wrongStreak = wrongStreak,
    difficultyEma = difficultyEma,
    lastResult = lastResult?.name,
    lastReviewedAt = lastReviewedAt,
    lastSeenSequence = lastSeenSequence,
    createdAt = createdAt,
    updatedAt = updatedAt,
)
