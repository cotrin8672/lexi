package io.github.cotrin8672.lexi.review.ui

import io.github.cotrin8672.lexi.review.MeaningOption
import io.github.cotrin8672.lexi.review.QuestionCandidate
import io.github.cotrin8672.lexi.review.ReviewMode
import io.github.cotrin8672.lexi.review.ReorderBankSlot
import io.github.cotrin8672.lexi.review.VocabularyListItem
import io.github.cotrin8672.lexi.review.availableReorderTokens
import io.github.cotrin8672.lexi.review.reorderBankSlots
import io.github.cotrin8672.lexi.review.schema.LexiResultV1
import io.github.cotrin8672.lexi.review.storage.VocabularySource

enum class SessionLoadPhase {
    MODE_SELECT,
    /** Waiting on first-time vocabulary download; distinct from cache reads. */
    SYNCING_VOCABULARY,
    READY,
    ERROR,
    VOCABULARY_LIST,
}

enum class QuestionInteractionPhase {
    ANSWERING,
    CHECKED,
    SKIPPED,
    /** Full dictionary-style word explanation after a wrong answer or skip. */
    WORD_DETAIL,
}

data class WordDetailContext(
    val content: LexiResultV1,
    val partOfSpeech: String? = null,
    val irregularForms: List<String> = emptyList(),
    val reviewHeadline: String,
    val reviewSubline: String? = null,
    val highlightTranslation: String? = null,
    val highlightExampleSentence: String? = null,
    val highlightSynonymTerm: String? = null,
)

sealed interface RenderedQuestion {
    val candidate: QuestionCandidate

    data class Meaning(
        override val candidate: QuestionCandidate,
        val headword: String,
        val options: List<MeaningOption>,
    ) : RenderedQuestion

    data class Reorder(
        override val candidate: QuestionCandidate,
        val promptJapanese: String,
        val bankOrder: List<String>,
        val originalSentence: String,
    ) : RenderedQuestion

    data class Usage(
        override val candidate: QuestionCandidate,
        val contextText: String,
        val options: List<MeaningOption>,
    ) : RenderedQuestion

    data class Inflection(
        override val candidate: QuestionCandidate,
        val primaryText: String,
        val expectedForm: String,
    ) : RenderedQuestion
}

data class ReviewUiState(
    val loadPhase: SessionLoadPhase = SessionLoadPhase.MODE_SELECT,
    val reviewMode: ReviewMode? = null,
    val vocabularySource: VocabularySource? = null,
    val errorMessage: String? = null,
    val sessionQuestionNumber: Int = 0,
    val totalCandidates: Int = 0,
    val interactionPhase: QuestionInteractionPhase = QuestionInteractionPhase.ANSWERING,
    val currentQuestion: RenderedQuestion? = null,
    val selectedOptionKey: String? = null,
    val inflectionAnswerText: String = "",
    val reorderBankOrder: List<String> = emptyList(),
    val reorderSelectedTokens: List<String> = emptyList(),
    val lastCheckCorrect: Boolean? = null,
    val wordDetailContext: WordDetailContext? = null,
    val sessionAnswered: Int = 0,
    val sessionCorrect: Int = 0,
    val vocabularyList: List<VocabularyListItem> = emptyList(),
    val vocabularyCount: Int = 0,
    val vocabularySyncInProgress: Boolean = false,
    val hasLocalCache: Boolean = false,
    val vocabularyCacheReady: Boolean = false,
    val vocabularySyncError: String? = null,
    val syncToastMessage: String? = null,
    val syncToastNonce: Int = 0,
) {
    fun reorderAvailableTokens(): List<String> =
        availableReorderTokens(reorderBankOrder, reorderSelectedTokens)

    fun reorderBankSlots(): List<ReorderBankSlot> =
        reorderBankSlots(reorderBankOrder, reorderSelectedTokens)
}

fun ReviewUiState.canCheckAnswer(): Boolean {
    if (interactionPhase != QuestionInteractionPhase.ANSWERING) {
        return false
    }
    return when (currentQuestion) {
        is RenderedQuestion.Reorder ->
            reorderAvailableTokens().isEmpty() && reorderSelectedTokens.isNotEmpty()
        is RenderedQuestion.Meaning,
        is RenderedQuestion.Usage,
        null -> false
        is RenderedQuestion.Inflection ->
            inflectionAnswerText.isNotBlank()
    }
}
