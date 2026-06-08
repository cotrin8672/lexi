package io.github.cotrin8672.lexi.review.speech

import io.github.cotrin8672.lexi.review.QuestionPayload
import io.github.cotrin8672.lexi.review.ui.RenderedQuestion

fun speakableHeadword(question: RenderedQuestion): String? =
    when (question) {
        is RenderedQuestion.Meaning -> question.headword.trim().takeIf { it.isNotEmpty() }
        is RenderedQuestion.Usage ->
            (question.candidate.payload as? QuestionPayload.Usage)
                ?.headword
                ?.trim()
                ?.takeIf { it.isNotEmpty() }
        is RenderedQuestion.Inflection ->
            (question.candidate.payload as? QuestionPayload.Inflection)
                ?.headword
                ?.trim()
                ?.takeIf { it.isNotEmpty() }
        is RenderedQuestion.Reorder -> null
    }

fun isMultipleChoiceQuestion(question: RenderedQuestion): Boolean =
    question is RenderedQuestion.Meaning ||
        question is RenderedQuestion.Usage
