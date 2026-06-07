package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import kotlin.random.Random
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class WeightingTest {
    private val cards = ReviewFixtures.vocabularyBundle().activeCards()
    private val candidates = extractQuestionCandidates(cards)

    @Test
    fun highDifficultyCandidateIsSampledMoreOften() {
        val target = candidates.first { it.questionType == QuestionType.MEANING }
        val easy = candidates.first { it.questionType == QuestionType.MEANING && it != target }
        val hardStats = QuestionStats(
            questionKey = target.questionKey,
            questionType = target.questionType,
            lexemeId = target.lexemeId,
            difficultyEma = 0.9,
            createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
        )
        val easyStats = QuestionStats(
            questionKey = easy.questionKey,
            questionType = easy.questionType,
            lexemeId = easy.lexemeId,
            difficultyEma = 0.1,
            createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
        )
        val pool = listOf(target, easy)
        val stats = mapOf(
            target.questionKey to hardStats,
            easy.questionKey to easyStats,
        )
        val random = Random(42)
        var hardHits = 0
        repeat(200) {
            val picked = sampleWeighted(pool, stats, WeightingContext(), random)
            if (picked?.questionKey == target.questionKey) {
                hardHits++
            }
        }
        assertTrue(hardHits > 120)
    }

    @Test
    fun recentMistakeBoostIncreasesWeight() {
        val candidate = candidates.first()
        val wrongStats = QuestionStats(
            questionKey = candidate.questionKey,
            questionType = candidate.questionType,
            lexemeId = candidate.lexemeId,
            lastResult = ReviewResult.WRONG,
            createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
        )
        val correctStats = wrongStats.copy(lastResult = ReviewResult.CORRECT)
        val wrongWeight = computeWeight(candidate, wrongStats, WeightingContext())
        val correctWeight = computeWeight(candidate, correctStats, WeightingContext())
        assertTrue(wrongWeight > correctWeight)
    }

    @Test
    fun freshnessPenaltyReducesRecentlySeenWeight() {
        val candidate = candidates.first()
        val fresh = computeWeight(candidate, null, WeightingContext())
        val penalized = computeWeight(
            candidate,
            null,
            WeightingContext(recentQuestionKeys = listOf(candidate.questionKey)),
        )
        assertTrue(penalized < fresh)
    }

    @Test
    fun smallCandidateSetStillProducesDraw() {
        val pool = candidates.take(2)
        val picked = sampleWeighted(pool, emptyMap(), WeightingContext(), Random(1))
        assertEquals(2, pool.size)
        assertTrue(picked != null)
        assertTrue(pool.contains(picked))
    }
}
