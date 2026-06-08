package io.github.cotrin8672.lexi.review.speech

import io.github.cotrin8672.lexi.review.InflectionDirection
import io.github.cotrin8672.lexi.review.QuestionCandidate
import io.github.cotrin8672.lexi.review.QuestionPayload
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.ui.RenderedQuestion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SpeakableTextTest {
    @Test
    fun speakableHeadwordReturnsEnglishLemmaForMultipleChoiceQuestions() {
        val meaning = RenderedQuestion.Meaning(
            candidate = meaningCandidate(),
            headword = "adopt",
            options = emptyList(),
        )
        val usage = RenderedQuestion.Usage(
            candidate = usageCandidate(),
            contextText = "context",
            options = emptyList(),
        )
        val inflection = RenderedQuestion.Inflection(
            candidate = inflectionCandidate(),
            primaryText = "adopted",
            expectedForm = "adopted",
        )

        assertEquals("adopt", speakableHeadword(meaning))
        assertEquals("adopt", speakableHeadword(usage))
        assertEquals("adopt", speakableHeadword(inflection))
    }

    @Test
    fun speakableHeadwordIsNullForReorderQuestions() {
        val reorder = RenderedQuestion.Reorder(
            candidate = QuestionCandidate(
                questionKey = "reorder:v1:lex-adopt:preview",
                questionType = QuestionType.REORDER,
                lexemeId = "lex-adopt",
                sourceHash = "preview",
                renderSeedHint = "preview",
                payload = QuestionPayload.Reorder(
                    promptJapanese = "example",
                    originalSentence = "The team adopted a policy.",
                    tokens = listOf("The", "team", "adopted", "a", "policy."),
                ),
            ),
            promptJapanese = "example",
            bankOrder = listOf("team", "The", "adopted"),
            originalSentence = "The team adopted a policy.",
        )

        assertNull(speakableHeadword(reorder))
        assertTrue(isMultipleChoiceQuestion(meaningCandidate().let {
            RenderedQuestion.Meaning(it, "adopt", emptyList())
        }))
    }

    private fun meaningCandidate() = QuestionCandidate(
        questionKey = "meaning:v1:lex-adopt:preview",
        questionType = QuestionType.MEANING,
        lexemeId = "lex-adopt",
        sourceHash = "preview",
        renderSeedHint = "preview",
        payload = QuestionPayload.Meaning(
            headword = "adopt",
            correctMeaning = "採用する",
            partOfSpeechNote = "動詞",
        ),
    )

    private fun usageCandidate() = QuestionCandidate(
        questionKey = "usage:v1:lex-adopt:preview",
        questionType = QuestionType.USAGE,
        lexemeId = "lex-adopt",
        sourceHash = "preview",
        renderSeedHint = "preview",
        payload = QuestionPayload.Usage(
            prompt = "prompt",
            correctTerm = "adopt",
            headword = "adopt",
            otherTerm = "adapt",
            comparison = "comparison",
        ),
    )

    private fun inflectionCandidate() = QuestionCandidate(
        questionKey = "inflection:v1:lex-adopt:preview",
        questionType = QuestionType.INFLECTION,
        lexemeId = "lex-adopt",
        sourceHash = "preview",
        renderSeedHint = "preview",
        payload = QuestionPayload.Inflection(
            headword = "adopt",
            formText = "adopted",
            relation = "past",
            formKey = "adopted",
            direction = InflectionDirection.FORM_TO_BASE,
        ),
    )
}
