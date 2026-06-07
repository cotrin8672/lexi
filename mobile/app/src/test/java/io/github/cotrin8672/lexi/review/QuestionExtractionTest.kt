package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class QuestionExtractionTest {
    private val cards = ReviewFixtures.vocabularyBundle().activeCards()

    @Test
    fun extractsMeaningReorderUsageAndInflectionCandidates() {
        val candidates = extractQuestionCandidates(cards)
        val types = candidates.map { it.questionType }.toSet()
        assertTrue(types.contains(QuestionType.MEANING))
        assertTrue(types.contains(QuestionType.REORDER))
        assertTrue(types.contains(QuestionType.USAGE))
        assertTrue(types.contains(QuestionType.INFLECTION))
    }

    @Test
    fun skipsMeaningWhenFewerThanThreeDistractors() {
        val senses = listOf(
            MeaningSense("lex-a", "採用する", "動詞"),
            MeaningSense("lex-b", "説明する", "動詞"),
        )
        val count = countMeaningDistractors(
            correctLexemeId = "lex-a",
            correctMeaning = "採用する",
            preferredNote = "動詞",
            meaningSenses = senses,
        )
        assertTrue(count < 3)
    }

    @Test
    fun skipsTooShortReorderSentences() {
        assertTrue(shouldSkipReorderSentence("Too short."))
        assertFalse(
            shouldSkipReorderSentence("The team adopted a new policy."),
        )
    }

    @Test
    fun skipsVagueUsageComparisons() {
        assertTrue(
            shouldSkipUsageComparison(
                comparison = "Similar words.",
                headword = "subtle",
                otherTerm = "delicate",
            ),
        )
        assertFalse(
            shouldSkipUsageComparison(
                comparison =
                    "Choose subtle for hard-to-notice differences; choose delicate for fine detail.",
                headword = "subtle",
                otherTerm = "delicate",
            ),
        )
    }

    @Test
    fun skipsInflectionFormsIdenticalToHeadword() {
        assertTrue(shouldSkipInflectionForm("go", "go"))
        assertFalse(shouldSkipInflectionForm("go", "went"))
    }
}
