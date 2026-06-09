package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.reorderBankSlots
import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.ui.QuestionInteractionPhase
import io.github.cotrin8672.lexi.review.ui.RenderedQuestion
import io.github.cotrin8672.lexi.review.ui.ReviewUiState
import io.github.cotrin8672.lexi.review.ui.SessionLoadPhase
import io.github.cotrin8672.lexi.review.ui.WordDetailContext
import kotlin.random.Random

class ReviewSessionEngine(
    private val random: Random = Random.Default,
    private val now: () -> String,
) {
    private var cards: List<ActiveVocabularyCard> = emptyList()
    private var candidates: List<QuestionCandidate> = emptyList()
    private var reviewMode: ReviewMode = ReviewMode.MIXED_RANDOM
    private val statsByKey = mutableMapOf<String, QuestionStats>()
    private val recentQuestionKeys = ArrayDeque<String>()
    private val recentLexemeIds = ArrayDeque<String>()
    private var seenSequence = 0L
    private var sessionQuestionNumber = 0
    private var sessionAnswered = 0
    private var sessionCorrect = 0
    private var currentCandidate: QuestionCandidate? = null

    private var _state = ReviewUiState()
    val state: ReviewUiState get() = _state

    fun startWithCards(
        cards: List<ActiveVocabularyCard>,
        source: VocabularySource,
        mode: ReviewMode = ReviewMode.MIXED_RANDOM,
        persistedStats: Map<String, QuestionStats> = emptyMap(),
    ): ReviewUiState {
        this.cards = cards
        this.reviewMode = mode
        this.candidates = extractQuestionCandidates(cards).filterForMode(mode)
        sessionQuestionNumber = 0
        sessionAnswered = 0
        sessionCorrect = 0
        seenSequence = 0L
        recentQuestionKeys.clear()
        recentLexemeIds.clear()
        statsByKey.clear()
        statsByKey.putAll(persistedStats)
        currentCandidate = null

        if (candidates.isEmpty()) {
            _state = ReviewUiState(
                loadPhase = SessionLoadPhase.ERROR,
                reviewMode = mode,
                vocabularySource = source,
                vocabularyCount = cards.size,
                errorMessage = "No review questions are available for this mode.",
            )
            return _state
        }

        _state = ReviewUiState(
            loadPhase = SessionLoadPhase.READY,
            reviewMode = mode,
            vocabularySource = source,
            vocabularyCount = cards.size,
            totalCandidates = candidates.size,
        )
        return advanceToNextQuestion()
    }

    fun updateInflectionAnswer(answerText: String): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        if (_state.currentQuestion !is RenderedQuestion.Inflection) {
            return _state
        }
        _state = _state.copy(inflectionAnswerText = answerText)
        return _state
    }

    fun selectOption(answerKey: String): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        _state = _state.copy(selectedOptionKey = answerKey)
        return _state
    }

    fun submitOption(answerKey: String): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        return when (_state.currentQuestion) {
            is RenderedQuestion.Meaning,
            is RenderedQuestion.Usage,
            -> {
                if (_state.selectedOptionKey == answerKey) {
                    checkAnswer()
                } else {
                    _state = _state.copy(selectedOptionKey = answerKey)
                    _state
                }
            }
            is RenderedQuestion.Inflection -> _state
            is RenderedQuestion.Reorder,
            null,
            -> _state
        }
    }

    fun addReorderToken(bankSlotIndex: Int): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        val bank = _state.reorderBankOrder
        if (bankSlotIndex !in bank.indices) {
            return _state
        }
        val slots = reorderBankSlots(bank, _state.reorderSelectedTokens)
        if (slots[bankSlotIndex].selected) {
            return _state
        }
        _state = _state.copy(
            reorderSelectedTokens = _state.reorderSelectedTokens + bank[bankSlotIndex],
        )
        return _state
    }

    fun removeReorderToken(selectedIndex: Int): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        val selected = _state.reorderSelectedTokens.toMutableList()
        if (selectedIndex !in selected.indices) {
            return _state
        }
        selected.removeAt(selectedIndex)
        _state = _state.copy(reorderSelectedTokens = selected)
        return _state
    }

    fun checkAnswer(): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        val candidate = currentCandidate ?: return _state
        val correct = evaluateAnswer(candidate)
            ?: return _state

        val updated = applyAnswer(
            existing = statsByKey[candidate.questionKey],
            candidate = candidate,
            correct = correct,
            reviewedAt = now(),
            seenSequence = seenSequence,
        )
        statsByKey[candidate.questionKey] = updated
        sessionAnswered += 1
        if (correct) {
            sessionCorrect += 1
        }

        _state = if (correct) {
            _state.copy(
                interactionPhase = QuestionInteractionPhase.CHECKED,
                lastCheckCorrect = true,
                wordDetailContext = null,
                sessionAnswered = sessionAnswered,
                sessionCorrect = sessionCorrect,
            )
        } else {
            _state.copy(
                interactionPhase = QuestionInteractionPhase.WORD_DETAIL,
                lastCheckCorrect = false,
                wordDetailContext = buildWordDetailContext(candidate),
                sessionAnswered = sessionAnswered,
                sessionCorrect = sessionCorrect,
            )
        }
        return _state
    }

    fun skipQuestion(): ReviewUiState {
        if (_state.interactionPhase != QuestionInteractionPhase.ANSWERING) {
            return _state
        }
        val candidate = currentCandidate
        _state = _state.copy(
            interactionPhase = QuestionInteractionPhase.WORD_DETAIL,
            lastCheckCorrect = null,
            wordDetailContext = candidate?.let { buildWordDetailContext(it) },
        )
        return _state
    }

    fun advanceToNextQuestion(): ReviewUiState {
        val rendered = sampleRenderedQuestion() ?: run {
            _state = _state.copy(
                loadPhase = SessionLoadPhase.ERROR,
                errorMessage = "Could not render a review question.",
            )
            return _state
        }

        sessionQuestionNumber += 1
        seenSequence += 1
        currentCandidate = rendered.candidate
        recordRecent(rendered.candidate)

        _state = _state.copy(
            loadPhase = SessionLoadPhase.READY,
            sessionQuestionNumber = sessionQuestionNumber,
            interactionPhase = QuestionInteractionPhase.ANSWERING,
            currentQuestion = rendered,
            selectedOptionKey = null,
            inflectionAnswerText = "",
            reorderBankOrder = when (rendered) {
                is RenderedQuestion.Reorder -> rendered.bankOrder
                else -> emptyList()
            },
            reorderSelectedTokens = emptyList(),
            lastCheckCorrect = null,
            wordDetailContext = null,
        )
        return _state
    }

    fun statsSnapshot(): Map<String, QuestionStats> = statsByKey.toMap()

    private fun sampleRenderedQuestion(): RenderedQuestion? {
        repeat(24) {
            val candidate = sampleWeighted(
                candidates = candidates,
                statsByKey = statsByKey,
                context = WeightingContext(
                    recentQuestionKeys = recentQuestionKeys.toList(),
                    recentLexemeIds = recentLexemeIds.toList(),
                ),
                random = random,
            ) ?: return null
            renderQuestion(candidate)?.let { return it }
        }
        return null
    }

    private fun renderQuestion(candidate: QuestionCandidate): RenderedQuestion? {
        val questionRandom = Random(
            candidate.renderSeedHint.hashCode() xor seenSequence.toInt(),
        )
        return when (candidate.questionType) {
            QuestionType.MEANING -> {
                val payload = candidate.payload as? QuestionPayload.Meaning ?: return null
                val options = generateMeaningOptions(candidate, cards, questionRandom) ?: return null
                RenderedQuestion.Meaning(
                    candidate = candidate,
                    headword = payload.headword,
                    options = options.options,
                )
            }
            QuestionType.REORDER -> {
                val payload = candidate.payload as? QuestionPayload.Reorder ?: return null
                val presentation = generateReorderPresentation(candidate, questionRandom) ?: return null
                RenderedQuestion.Reorder(
                    candidate = candidate,
                    promptJapanese = payload.promptJapanese,
                    bankOrder = presentation.shuffledTokens,
                    originalSentence = payload.originalSentence,
                )
            }
            QuestionType.USAGE -> {
                val payload = candidate.payload as? QuestionPayload.Usage ?: return null
                val options = generateUsageOptions(candidate, cards, questionRandom) ?: return null
                RenderedQuestion.Usage(
                    candidate = candidate,
                    contextText = payload.prompt,
                    options = options,
                )
            }
            QuestionType.INFLECTION -> {
                val payload = candidate.payload as? QuestionPayload.Inflection ?: return null
                RenderedQuestion.Inflection(
                    candidate = candidate,
                    primaryText = inflectionPrimaryText(payload),
                    expectedForm = payload.formText,
                )
            }
        }
    }

    private fun evaluateAnswer(candidate: QuestionCandidate): Boolean? {
        return when (val question = _state.currentQuestion) {
            is RenderedQuestion.Meaning,
            is RenderedQuestion.Usage,
            -> {
                val selectedKey = _state.selectedOptionKey ?: return null
                val options = when (question) {
                    is RenderedQuestion.Meaning -> question.options
                    is RenderedQuestion.Usage -> question.options
                    is RenderedQuestion.Inflection,
                    is RenderedQuestion.Reorder,
                    -> return null
                }
                options.firstOrNull { it.answerKey == selectedKey }?.isCorrect == true
            }
            is RenderedQuestion.Inflection -> {
                val payload = candidate.payload as? QuestionPayload.Inflection ?: return null
                if (_state.inflectionAnswerText.isBlank()) {
                    return null
                }
                isInflectionAnswerCorrect(payload, _state.inflectionAnswerText)
            }
            is RenderedQuestion.Reorder -> {
                if (_state.reorderAvailableTokens().isNotEmpty()) {
                    return null
                }
                isReorderAnswerCorrect(
                    originalSentence = question.originalSentence,
                    submittedTokens = _state.reorderSelectedTokens,
                )
            }
            null -> null
        }
    }

    private fun buildWordDetailContext(candidate: QuestionCandidate): WordDetailContext? {
        val card = cards.firstOrNull { it.lexemeId == candidate.lexemeId } ?: return null
        val content = card.content
        val irregularForms = buildList {
            content.inflections.forEach { inflection ->
                if (inflection.form != content.headword) {
                    add(inflection.form)
                }
            }
            card.forms.filter { it.relation == "irregular" }.forEach { form ->
                if (form.formText != content.headword) {
                    add(form.formText)
                }
            }
        }.distinct()
        val summary = reviewSummaryFor(candidate)
        return WordDetailContext(
            content = content,
            partOfSpeech = card.lexeme.partOfSpeech,
            irregularForms = irregularForms,
            reviewHeadline = summary.headline,
            reviewSubline = summary.subline,
            highlightTranslation = summary.highlightTranslation,
            highlightExampleSentence = summary.highlightExampleSentence,
            highlightSynonymTerm = summary.highlightSynonymTerm,
        )
    }

    private data class ReviewSummary(
        val headline: String,
        val subline: String?,
        val highlightTranslation: String? = null,
        val highlightExampleSentence: String? = null,
        val highlightSynonymTerm: String? = null,
    )

    private fun reviewSummaryFor(candidate: QuestionCandidate): ReviewSummary =
        when (val payload = candidate.payload) {
            is QuestionPayload.Meaning -> ReviewSummary(
                headline = "正解: ${payload.correctMeaning}",
                subline = payload.partOfSpeechNote,
                highlightTranslation = payload.correctMeaning,
            )
            is QuestionPayload.Usage -> ReviewSummary(
                headline = "正解: ${payload.correctTerm}",
                subline = payload.comparison,
                highlightSynonymTerm = payload.correctTerm,
            )
            is QuestionPayload.Inflection -> ReviewSummary(
                headline = "正解: ${payload.formText}",
                subline = formatInflectionRelationLabel(payload.relation),
            )
            is QuestionPayload.Reorder -> ReviewSummary(
                headline = payload.originalSentence,
                subline = payload.promptJapanese,
                highlightExampleSentence = payload.originalSentence,
            )
        }

    private fun recordRecent(candidate: QuestionCandidate) {
        recentQuestionKeys.addFirst(candidate.questionKey)
        recentLexemeIds.addFirst(candidate.lexemeId)
        while (recentQuestionKeys.size > 10) {
            recentQuestionKeys.removeLast()
        }
        while (recentLexemeIds.size > 10) {
            recentLexemeIds.removeLast()
        }
    }
}

private fun formatInflectionRelationLabel(relation: String): String =
    when (relation.lowercase()) {
        "past" -> "過去形"
        "pastparticiple", "past_participle" -> "過去分詞"
        "plural" -> "複数形"
        "irregular" -> "不規則変化"
        else -> relation.replace('_', ' ').trim()
    }

private fun inflectionPrimaryText(payload: QuestionPayload.Inflection): String =
    when (payload.direction) {
        InflectionDirection.BASE_TO_FORM -> payload.headword
        InflectionDirection.FORM_TO_BASE,
        InflectionDirection.KIND_RECOGNITION,
        -> payload.formText
    }
