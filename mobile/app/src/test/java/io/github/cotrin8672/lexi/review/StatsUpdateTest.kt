package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class StatsUpdateTest {
    private val candidate = extractQuestionCandidates(
        ReviewFixtures.vocabularyBundle().activeCards(),
    ).first()

    @Test
    fun wrongAnswerIncreasesDifficultyEma() {
        val updated = applyAnswer(
            existing = null,
            candidate = candidate,
            correct = false,
            reviewedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            seenSequence = 1,
        )
        assertTrue(updated.difficultyEma > 0.5)
        assertEquals(ReviewResult.WRONG, updated.lastResult)
        assertEquals(1, updated.wrongStreak)
        assertEquals(0, updated.correctStreak)
    }

    @Test
    fun correctAnswerDecreasesDifficultyEmaGradually() {
        val afterWrong = applyAnswer(
            existing = null,
            candidate = candidate,
            correct = false,
            reviewedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            seenSequence = 1,
        )
        val afterCorrect = applyAnswer(
            existing = afterWrong,
            candidate = candidate,
            correct = true,
            reviewedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            seenSequence = 2,
        )
        assertTrue(afterCorrect.difficultyEma < afterWrong.difficultyEma)
        assertEquals(ReviewResult.CORRECT, afterCorrect.lastResult)
        assertEquals(1, afterCorrect.correctStreak)
        assertEquals(0, afterCorrect.wrongStreak)
    }

    @Test
    fun streaksResetInCorrectDirection() {
        val wrongTwice = applyAnswer(
            existing = applyAnswer(null, candidate, false, ReviewFixtures.FIXTURE_TIMESTAMP, 1),
            candidate = candidate,
            correct = false,
            reviewedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            seenSequence = 2,
        )
        assertEquals(2, wrongTwice.wrongStreak)

        val corrected = applyAnswer(
            existing = wrongTwice,
            candidate = candidate,
            correct = true,
            reviewedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            seenSequence = 3,
        )
        assertEquals(0, corrected.wrongStreak)
        assertEquals(1, corrected.correctStreak)
    }
}
