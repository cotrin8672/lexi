package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ReviewModeTest {
    private val candidates = extractQuestionCandidates(ReviewFixtures.vocabularyBundle().activeCards())

    @Test
    fun filterForModeKeepsOnlyMatchingType() {
        val meaningOnly = candidates.filterForMode(ReviewMode.MEANING_ONLY)
        assertTrue(meaningOnly.isNotEmpty())
        assertTrue(meaningOnly.all { it.questionType == QuestionType.MEANING })

        val reorderOnly = candidates.filterForMode(ReviewMode.REORDER_ONLY)
        assertTrue(reorderOnly.all { it.questionType == QuestionType.REORDER })
    }

    @Test
    fun mixedModeKeepsAllCandidates() {
        assertEquals(candidates.size, candidates.filterForMode(ReviewMode.MIXED_RANDOM).size)
    }
}
