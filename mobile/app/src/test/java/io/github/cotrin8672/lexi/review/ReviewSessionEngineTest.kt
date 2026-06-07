package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.ui.QuestionInteractionPhase
import io.github.cotrin8672.lexi.review.ui.RenderedQuestion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReviewSessionEngineTest {
    private val cards = ReviewFixtures.vocabularyBundle().activeCards()

    @Test
    fun startsWithFixtureCandidatesAndRendersFirstQuestion() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        val state = engine.startWithCards(cards, VocabularySource.FIXTURE)

        assertTrue(state.totalCandidates > 0)
        assertEquals(1, state.sessionQuestionNumber)
        assertTrue(state.currentQuestion != null)
        assertEquals(QuestionInteractionPhase.ANSWERING, state.interactionPhase)
    }

    @Test
    fun checkUpdatesStatsButSkipDoesNot() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        engine.startWithCards(cards, VocabularySource.FIXTURE)
        val questionKey = engine.state.currentQuestion?.candidate?.questionKey
            ?: error("expected a rendered question")

        forceCorrectAnswer(engine)
        engine.checkAnswer()
        assertEquals(QuestionInteractionPhase.CHECKED, engine.state.interactionPhase)
        assertEquals(1, engine.state.sessionAnswered)
        assertTrue(engine.statsSnapshot().containsKey(questionKey))

        engine.advanceToNextQuestion()
        val skippedKey = engine.state.currentQuestion?.candidate?.questionKey
            ?: error("expected another question")
        engine.skipQuestion()
        engine.advanceToNextQuestion()

        assertEquals(1, engine.statsSnapshot()[questionKey]?.attempts)
        assertNull(engine.statsSnapshot()[skippedKey])
    }

    @Test
    fun meaningOnlyModeFiltersCandidates() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        val state = engine.startWithCards(cards, VocabularySource.FIXTURE, ReviewMode.MEANING_ONLY)

        assertTrue(state.totalCandidates > 0)
        repeat(6) {
            assertTrue(engine.state.currentQuestion is RenderedQuestion.Meaning)
            forceCorrectAnswer(engine)
            engine.checkAnswer()
            engine.advanceToNextQuestion()
        }
    }

    @Test
    fun reorderBankKeepsStableSlotPositionsWhenTokensMove() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        engine.startWithCards(cards, VocabularySource.FIXTURE, ReviewMode.REORDER_ONLY)
        val initialBank = engine.state.reorderBankOrder
        assertTrue(initialBank.isNotEmpty())

        engine.addReorderToken(0)
        val selectedToken = initialBank.first()
        assertEquals(
            availableReorderTokens(initialBank, listOf(selectedToken)),
            engine.state.reorderAvailableTokens(),
        )
        assertEquals(initialBank.size, engine.state.reorderBankSlots().size)
        assertTrue(engine.state.reorderBankSlots()[0].selected)

        engine.removeReorderToken(0)
        assertEquals(initialBank, engine.state.reorderAvailableTokens())
        assertTrue(engine.state.reorderBankSlots().none { it.selected })
    }

    @Test
    fun submitOptionRequiresTwoTapsOnSameMeaningChoice() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        engine.startWithCards(cards, VocabularySource.FIXTURE, ReviewMode.MEANING_ONLY)

        val correctKey = when (val question = engine.state.currentQuestion) {
            is RenderedQuestion.Meaning -> question.options.first { it.isCorrect }.answerKey
            else -> error("expected meaning question")
        }

        engine.submitOption(correctKey)
        assertEquals(QuestionInteractionPhase.ANSWERING, engine.state.interactionPhase)
        assertEquals(correctKey, engine.state.selectedOptionKey)
        assertNull(engine.state.lastCheckCorrect)
        assertEquals(0, engine.state.sessionAnswered)

        engine.submitOption(correctKey)
        assertEquals(QuestionInteractionPhase.CHECKED, engine.state.interactionPhase)
        assertEquals(true, engine.state.lastCheckCorrect)
        assertEquals(1, engine.state.sessionAnswered)
    }

    @Test
    fun submitOptionSecondTapOnDifferentChoiceOnlyChangesSelection() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        engine.startWithCards(cards, VocabularySource.FIXTURE, ReviewMode.MEANING_ONLY)

        val (firstKey, secondKey) = when (val question = engine.state.currentQuestion) {
            is RenderedQuestion.Meaning -> {
                val first = question.options[0].answerKey
                val second = question.options[1].answerKey
                first to second
            }
            else -> error("expected meaning question")
        }

        engine.submitOption(firstKey)
        assertEquals(QuestionInteractionPhase.ANSWERING, engine.state.interactionPhase)
        assertEquals(firstKey, engine.state.selectedOptionKey)

        engine.submitOption(secondKey)
        assertEquals(QuestionInteractionPhase.ANSWERING, engine.state.interactionPhase)
        assertEquals(secondKey, engine.state.selectedOptionKey)
        assertNull(engine.state.lastCheckCorrect)
        assertEquals(0, engine.state.sessionAnswered)
    }

    @Test
    fun startWithCardsHydratesPersistedStatsForWeighting() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        val candidates = extractQuestionCandidates(cards)
        val persistedKey = candidates.first().questionKey
        val persisted = mapOf(
            persistedKey to QuestionStats(
                questionKey = persistedKey,
                questionType = candidates.first().questionType,
                lexemeId = candidates.first().lexemeId,
                difficultyEma = 0.95,
                wrongCount = 4,
                lastResult = ReviewResult.WRONG,
                createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
                updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            ),
        )

        engine.startWithCards(
            cards = cards,
            source = VocabularySource.FIXTURE,
            persistedStats = persisted,
        )

        val hydrated = engine.statsSnapshot()[persistedKey]
        assertTrue(hydrated != null)
        assertEquals(0.95, hydrated!!.difficultyEma, 0.0001)
        assertEquals(ReviewResult.WRONG, hydrated.lastResult)
    }

    @Test
    fun wrongAnswerProvidesLearningContext() {
        val engine = ReviewSessionEngine(now = { ReviewFixtures.FIXTURE_TIMESTAMP })
        engine.startWithCards(cards, VocabularySource.FIXTURE, ReviewMode.MEANING_ONLY)

        when (val question = engine.state.currentQuestion) {
            is RenderedQuestion.Meaning -> {
                val wrong = question.options.first { !it.isCorrect }
                engine.selectOption(wrong.answerKey)
            }
            else -> error("expected meaning question")
        }
        engine.checkAnswer()

        val context = engine.state.wrongAnswerContext
        assertTrue(context != null)
        assertTrue(!context!!.correctAnswer.isBlank())
    }

    private fun forceCorrectAnswer(engine: ReviewSessionEngine) {
        when (val question = engine.state.currentQuestion) {
            is RenderedQuestion.Meaning,
            is RenderedQuestion.Usage,
            is RenderedQuestion.Inflection,
            -> {
                val options = when (question) {
                    is RenderedQuestion.Meaning -> question.options
                    is RenderedQuestion.Usage -> question.options
                    is RenderedQuestion.Inflection -> question.options
                }
                val correct = options.first { it.isCorrect }
                engine.selectOption(correct.answerKey)
            }
            is RenderedQuestion.Reorder -> {
                question.bankOrder.indices.forEach { index ->
                    engine.addReorderToken(index)
                }
            }
            null -> error("expected a rendered question")
        }
    }
}
