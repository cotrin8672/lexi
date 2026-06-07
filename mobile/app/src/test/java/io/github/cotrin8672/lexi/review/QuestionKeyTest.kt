package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import kotlin.random.Random
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class QuestionKeyTest {
    private val cards = ReviewFixtures.vocabularyBundle().activeCards()

    @Test
    fun meaningKeyIsStableAcrossTranslationOrder() {
        val keyA = QuestionKey.meaning("lex-adopt", "採用する")
        val keyB = QuestionKey.meaning("lex-adopt", "  採用する  ")
        assertEquals(keyA, keyB)
    }

    @Test
    fun meaningOptionsDoNotChangeQuestionKey() {
        val candidates = extractQuestionCandidates(cards)
        val meaning = candidates.first { it.questionType == QuestionType.MEANING }
        val optionsA = generateMeaningOptions(meaning, cards, Random(11))
        val optionsB = generateMeaningOptions(meaning, cards, Random(29))
        requireNotNull(optionsA)
        requireNotNull(optionsB)
        assertEquals(meaning.questionKey, optionsA.questionKey)
        assertEquals(meaning.questionKey, optionsB.questionKey)
        assertNotEquals(optionsA.options.map { it.label }, optionsB.options.map { it.label })
    }

    @Test
    fun reorderKeyIsStableAcrossTokenShuffle() {
        val candidates = extractQuestionCandidates(cards)
        val reorder = candidates.first { it.questionType == QuestionType.REORDER }
        val presentationA = generateReorderPresentation(reorder, Random(3))
        val presentationB = generateReorderPresentation(reorder, Random(7))
        requireNotNull(presentationA)
        requireNotNull(presentationB)
        assertEquals(reorder.questionKey, presentationA.questionKey)
        assertEquals(reorder.questionKey, presentationB.questionKey)
    }

    @Test
    fun differentMeaningsProduceDifferentKeys() {
        val adopt = QuestionKey.meaning("lex-adopt", "採用する")
        val explain = QuestionKey.meaning("lex-explain", "説明する")
        assertNotEquals(adopt, explain)
    }
}
