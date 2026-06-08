package io.github.cotrin8672.lexi.review

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OptionGenerationTest {
    @Test
    fun inflectionAnswerMatchesCaseInsensitively() {
        val payload = QuestionPayload.Inflection(
            headword = "go",
            formText = "went",
            relation = "past",
            formKey = "went",
            direction = InflectionDirection.BASE_TO_FORM,
        )

        assertTrue(isInflectionAnswerCorrect(payload, "went"))
        assertTrue(isInflectionAnswerCorrect(payload, " Went "))
        assertFalse(isInflectionAnswerCorrect(payload, "go"))
    }
}
