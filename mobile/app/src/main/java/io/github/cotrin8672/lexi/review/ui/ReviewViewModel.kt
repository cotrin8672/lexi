package io.github.cotrin8672.lexi.review.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import io.github.cotrin8672.lexi.review.AppDependencies
import io.github.cotrin8672.lexi.review.ReviewMode
import io.github.cotrin8672.lexi.review.ReviewSessionEngine
import io.github.cotrin8672.lexi.review.extractQuestionCandidates
import io.github.cotrin8672.lexi.review.filterForMode
import io.github.cotrin8672.lexi.review.loadSessionVocabulary
import io.github.cotrin8672.lexi.review.storage.InMemoryReviewStore
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.toVocabularyListItems
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant

class ReviewViewModel(
    private val vocabularyRepository: VocabularyRepository,
    private val reviewStore: ReviewStore = InMemoryReviewStore(),
    private val sessionUserId: () -> String? = { null },
    private val canRefreshFromSupabase: () -> Boolean = { false },
) : ViewModel() {
    private val engine = ReviewSessionEngine(
        now = { Instant.now().toString() },
    )

    private val _uiState = MutableStateFlow(ReviewUiState())
    val uiState: StateFlow<ReviewUiState> = _uiState.asStateFlow()

    private var pendingRetryMode: ReviewMode? = null
    private var pendingVocabularyList = false

    fun startSession(mode: ReviewMode) {
        pendingRetryMode = mode
        pendingVocabularyList = false
        viewModelScope.launch {
            _uiState.value = ReviewUiState(
                loadPhase = SessionLoadPhase.LOADING,
                reviewMode = mode,
            )
            when (val result = loadSessionVocabulary(
                repository = vocabularyRepository,
                userId = sessionUserId(),
                canRefreshFromSupabase = canRefreshFromSupabase(),
            )) {
                is VocabularyLoadResult.Success -> {
                    val candidateKeys = extractQuestionCandidates(result.cards)
                        .filterForMode(mode)
                        .map { it.questionKey }
                    val persisted = reviewStore.getStatsByKeys(candidateKeys)
                    publish(
                        engine.startWithCards(
                            cards = result.cards,
                            source = result.source,
                            mode = mode,
                            persistedStats = persisted,
                        ),
                    )
                }
                is VocabularyLoadResult.Failure -> {
                    _uiState.value = ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        reviewMode = mode,
                        errorMessage = result.message,
                    )
                }
            }
        }
    }

    fun loadVocabularyList() {
        pendingVocabularyList = true
        pendingRetryMode = null
        viewModelScope.launch {
            _uiState.value = ReviewUiState(loadPhase = SessionLoadPhase.LOADING)
            when (val result = loadSessionVocabulary(
                repository = vocabularyRepository,
                userId = sessionUserId(),
                canRefreshFromSupabase = canRefreshFromSupabase(),
            )) {
                is VocabularyLoadResult.Success -> {
                    _uiState.value = ReviewUiState(
                        loadPhase = SessionLoadPhase.VOCABULARY_LIST,
                        vocabularySource = result.source,
                        vocabularyList = result.cards.toVocabularyListItems(),
                        vocabularyCount = result.cards.size,
                    )
                }
                is VocabularyLoadResult.Failure -> {
                    _uiState.value = ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        errorMessage = result.message,
                    )
                }
            }
        }
    }

    /** Fixture-only session for previews and tests. */
    fun loadFixtureSession() {
        pendingRetryMode = ReviewMode.MIXED_RANDOM
        pendingVocabularyList = false
        viewModelScope.launch {
            _uiState.value = ReviewUiState(
                loadPhase = SessionLoadPhase.LOADING,
                reviewMode = ReviewMode.MIXED_RANDOM,
            )
            when (val result = vocabularyRepository.loadFixtureCards()) {
                is VocabularyLoadResult.Success -> {
                    publish(
                        engine.startWithCards(
                            cards = result.cards,
                            source = VocabularySource.FIXTURE,
                            mode = ReviewMode.MIXED_RANDOM,
                        ),
                    )
                }
                is VocabularyLoadResult.Failure -> {
                    _uiState.value = ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        reviewMode = ReviewMode.MIXED_RANDOM,
                        errorMessage = result.message,
                    )
                }
            }
        }
    }

    fun retryLastLoad() {
        when {
            pendingVocabularyList -> loadVocabularyList()
            pendingRetryMode != null -> startSession(pendingRetryMode!!)
            else -> loadVocabularyList()
        }
    }

    fun returnToModeSelect() {
        pendingRetryMode = null
        pendingVocabularyList = false
        _uiState.value = ReviewUiState(loadPhase = SessionLoadPhase.MODE_SELECT)
    }

    /**
     * Read-only Supabase vocabulary refresh. Review stats are never pushed upstream.
     */
    fun tryRefreshFromSupabase(userId: String? = sessionUserId()) {
        val resolvedUserId = userId?.takeIf { it.isNotBlank() }
        if (resolvedUserId == null) {
            _uiState.value = _uiState.value.copy(
                loadPhase = SessionLoadPhase.ERROR,
                errorMessage = "Sign in to refresh vocabulary from Supabase.",
            )
            return
        }
        viewModelScope.launch {
            val mode = _uiState.value.reviewMode ?: ReviewMode.MIXED_RANDOM
            when (val result = vocabularyRepository.refreshFromSupabase(resolvedUserId)) {
                is VocabularyLoadResult.Success -> {
                    val candidateKeys = extractQuestionCandidates(result.cards)
                        .filterForMode(mode)
                        .map { it.questionKey }
                    val persisted = reviewStore.getStatsByKeys(candidateKeys)
                    publish(
                        engine.startWithCards(
                            cards = result.cards,
                            source = result.source,
                            mode = mode,
                            persistedStats = persisted,
                        ),
                    )
                }
                is VocabularyLoadResult.Failure -> {
                    _uiState.value = _uiState.value.copy(
                        loadPhase = SessionLoadPhase.ERROR,
                        errorMessage = result.message,
                    )
                }
            }
        }
    }

    fun selectOption(answerKey: String) {
        publish(engine.selectOption(answerKey))
    }

    fun submitOption(answerKey: String) {
        viewModelScope.launch {
            val beforeKey = engine.state.currentQuestion?.candidate?.questionKey
            publish(engine.submitOption(answerKey))
            val afterKey = engine.state.currentQuestion?.candidate?.questionKey
            if (beforeKey != null && beforeKey == afterKey) {
                engine.statsSnapshot()[beforeKey]?.let { reviewStore.upsertStats(it) }
            }
        }
    }

    fun addReorderToken(bankSlotIndex: Int) {
        publish(engine.addReorderToken(bankSlotIndex))
    }

    fun removeReorderToken(selectedIndex: Int) {
        publish(engine.removeReorderToken(selectedIndex))
    }

    fun checkAnswer() {
        viewModelScope.launch {
            val beforeKey = engine.state.currentQuestion?.candidate?.questionKey
            publish(engine.checkAnswer())
            val afterKey = engine.state.currentQuestion?.candidate?.questionKey
            if (beforeKey != null && beforeKey == afterKey) {
                engine.statsSnapshot()[beforeKey]?.let { reviewStore.upsertStats(it) }
            }
        }
    }

    fun skipQuestion() {
        publish(engine.skipQuestion())
    }

    fun nextQuestion() {
        publish(engine.advanceToNextQuestion())
    }

    private fun publish(state: ReviewUiState) {
        _uiState.update { state }
    }

    companion object {
        fun factory(dependencies: AppDependencies): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    ReviewViewModel(
                        vocabularyRepository = dependencies.vocabularyRepository,
                        reviewStore = dependencies.reviewStore,
                        sessionUserId = dependencies::activeUserId,
                        canRefreshFromSupabase = dependencies::canRefreshFromSupabase,
                    )
                }
            }

        fun factory(vocabularyRepository: VocabularyRepository): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    ReviewViewModel(vocabularyRepository = vocabularyRepository)
                }
            }
    }
}
