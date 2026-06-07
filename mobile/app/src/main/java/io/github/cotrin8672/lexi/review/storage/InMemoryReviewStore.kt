package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.QuestionStats

class InMemoryReviewStore : ReviewStore {
    private val stats = mutableMapOf<String, QuestionStats>()

    override suspend fun getStats(questionKey: String): QuestionStats? = stats[questionKey]

    override suspend fun getAllStats(): List<QuestionStats> = stats.values.toList()

    override suspend fun getStatsByKeys(questionKeys: Collection<String>): Map<String, QuestionStats> =
        questionKeys.mapNotNull { key -> stats[key]?.let { key to it } }.toMap()

    override suspend fun upsertStats(stats: QuestionStats) {
        this.stats[stats.questionKey] = stats
    }
}
