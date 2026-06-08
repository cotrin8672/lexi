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
import io.github.cotrin8672.lexi.review.speech.NoOpWordSpeech
import io.github.cotrin8672.lexi.review.speech.WordSpeech
import io.github.cotrin8672.lexi.review.speech.speakableHeadword
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.sync.VocabularySyncCoordinator
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
    private val wordSpeech: WordSpeech = NoOpWordSpeech,
    private val sessionUserId: () -> String? = { null },
    private val canRefreshFromSupabase: () -> Boolean = { false },
    private val vocabularySync: VocabularySyncCoordinator? = null,
) : ViewModel() {
    private val engine = ReviewSessionEngine(
        now = { Instant.now().toString() },
    )

    private val _uiState = MutableStateFlow(ReviewUiState())
    val uiState: StateFlow<ReviewUiState> = _uiState.asStateFlow()

    private var pendingRetryMode: ReviewMode? = null
    private var pendingVocabularyList = false

    init {
        vocabularySync?.let { coordinator ->
            viewModelScope.launch {
                coordinator.status.collect { syncStatus ->
                    _uiState.update { current ->
                        if (current.loadPhase != SessionLoadPhase.MODE_SELECT) {
                            current
                        } else {
                            current.copy(
                                vocabularySyncInProgress = syncStatus.isSyncing,
                                vocabularyCacheReady = syncStatus.cacheReady,
                            )
                        }
                    }
                }
            }
        }
    }

    fun onSignedIn() {
        val userId = sessionUserId()?.takeIf { it.isNotBlank() } ?: return
        vocabularySync?.probeCache(userId)
        vocabularySync?.scheduleSync(userId)
    }

    fun startSession(mode: ReviewMode) {
        pendingRetryMode = mode
        pendingVocabularyList = false
        viewModelScope.launch {
            val userId = sessionUserId()?.takeIf { it.isNotBlank() }
            if (userId == null) {
                _uiState.value = ReviewUiState(
                    loadPhase = SessionLoadPhase.ERROR,
                    reviewMode = mode,
                    errorMessage = "No account on this device. Sign in to sync vocabulary.",
                )
                return@launch
            }
            loadAndStartSession(mode, userId)
        }
    }

    fun loadVocabularyList() {
        pendingVocabularyList = true
        pendingRetryMode = null
        viewModelScope.launch {
            val userId = sessionUserId()?.takeIf { it.isNotBlank() }
            if (userId == null) {
                _uiState.value = ReviewUiState(
                    loadPhase = SessionLoadPhase.ERROR,
                    errorMessage = "No account on this device. Sign in to sync vocabulary.",
                )
                return@launch
            }
            loadAndShowVocabularyList(userId)
        }
    }

    /** Fixture-only session for previews and tests. */
    fun loadFixtureSession() {
        pendingRetryMode = ReviewMode.MIXED_RANDOM
        pendingVocabularyList = false
        viewModelScope.launch {
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
        val syncStatus = vocabularySync?.status?.value
        _uiState.value = ReviewUiState(
            loadPhase = SessionLoadPhase.MODE_SELECT,
            vocabularySyncInProgress = syncStatus?.isSyncing == true,
            vocabularyCacheReady = syncStatus?.cacheReady == true,
        )
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
            _uiState.value = ReviewUiState(
                loadPhase = SessionLoadPhase.SYNCING_VOCABULARY,
                reviewMode = mode,
            )
            vocabularySync?.syncNow(resolvedUserId)
            when (val result = loadSessionVocabulary(vocabularyRepository, resolvedUserId)) {
                is VocabularyLoadResult.Success -> publishSession(mode, result)
                is VocabularyLoadResult.Failure -> {
                    _uiState.value = ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        reviewMode = mode,
                        errorMessage = vocabularySync?.status?.value?.lastError ?: result.message,
                    )
                }
            }
        }
    }

    fun updateInflectionAnswer(answerText: String) {
        publish(engine.updateInflectionAnswer(answerText))
    }

    fun selectOption(answerKey: String) {
        publish(engine.selectOption(answerKey))
    }

    fun submitOption(answerKey: String) {
        viewModelScope.launch {
            val beforeKey = engine.state.currentQuestion?.candidate?.questionKey
            val beforePhase = engine.state.interactionPhase
            publish(engine.submitOption(answerKey))
            speakAfterMultipleChoiceCheck(beforePhase)
            val afterKey = engine.state.currentQuestion?.candidate?.questionKey
            if (beforeKey != null && beforeKey == afterKey) {
                engine.statsSnapshot()[beforeKey]?.let { reviewStore.upsertStats(it) }
            }
        }
    }

    fun addReorderToken(bankSlotIndex: Int) {
        val beforeCount = engine.state.reorderSelectedTokens.size
        publish(engine.addReorderToken(bankSlotIndex))
        val addedToken = engine.state.reorderSelectedTokens
            .drop(beforeCount)
            .firstOrNull()
        if (addedToken != null) {
            wordSpeech.speak(addedToken)
        }
    }

    fun removeReorderToken(selectedIndex: Int) {
        publish(engine.removeReorderToken(selectedIndex))
    }

    fun checkAnswer() {
        viewModelScope.launch {
            val beforeKey = engine.state.currentQuestion?.candidate?.questionKey
            val beforePhase = engine.state.interactionPhase
            publish(engine.checkAnswer())
            speakAfterMultipleChoiceCheck(beforePhase)
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

    private suspend fun loadAndStartSession(mode: ReviewMode, userId: String) {
        when (val result = loadSessionVocabulary(vocabularyRepository, userId)) {
            is VocabularyLoadResult.Success -> publishSession(mode, result)
            is VocabularyLoadResult.Failure -> waitForSyncAndStartSession(mode, userId, result.message)
        }
    }

    private suspend fun loadAndShowVocabularyList(userId: String) {
        when (val result = loadSessionVocabulary(vocabularyRepository, userId)) {
            is VocabularyLoadResult.Success -> {
                _uiState.value = ReviewUiState(
                    loadPhase = SessionLoadPhase.VOCABULARY_LIST,
                    vocabularySource = result.source,
                    vocabularyList = result.cards.toVocabularyListItems(),
                    vocabularyCount = result.cards.size,
                )
            }
            is VocabularyLoadResult.Failure -> waitForSyncAndShowVocabularyList(userId, result.message)
        }
    }

    private suspend fun waitForSyncAndStartSession(
        mode: ReviewMode,
        userId: String,
        cacheMessage: String,
    ) {
        if (!canRefreshFromSupabase() || vocabularySync == null) {
            _uiState.value = ReviewUiState(
                loadPhase = SessionLoadPhase.ERROR,
                reviewMode = mode,
                errorMessage = cacheMessage,
            )
            return
        }

        _uiState.value = ReviewUiState(
            loadPhase = SessionLoadPhase.SYNCING_VOCABULARY,
            reviewMode = mode,
        )
        vocabularySync.syncNow(userId)

        when (val result = loadSessionVocabulary(vocabularyRepository, userId)) {
            is VocabularyLoadResult.Success -> publishSession(mode, result)
            is VocabularyLoadResult.Failure -> {
                _uiState.value = ReviewUiState(
                    loadPhase = SessionLoadPhase.ERROR,
                    reviewMode = mode,
                    errorMessage = vocabularySync.status.value.lastError ?: result.message,
                )
            }
        }
    }

    private suspend fun waitForSyncAndShowVocabularyList(userId: String, cacheMessage: String) {
        if (!canRefreshFromSupabase() || vocabularySync == null) {
            _uiState.value = ReviewUiState(
                loadPhase = SessionLoadPhase.ERROR,
                errorMessage = cacheMessage,
            )
            return
        }

        _uiState.value = ReviewUiState(loadPhase = SessionLoadPhase.SYNCING_VOCABULARY)
        vocabularySync.syncNow(userId)

        when (val result = loadSessionVocabulary(vocabularyRepository, userId)) {
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
                    errorMessage = vocabularySync.status.value.lastError ?: result.message,
                )
            }
        }
    }

    private suspend fun publishSession(mode: ReviewMode, result: VocabularyLoadResult.Success) {
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

    private fun publish(state: ReviewUiState) {
        _uiState.update { state }
    }

    private fun speakAfterMultipleChoiceCheck(beforePhase: QuestionInteractionPhase) {
        if (beforePhase != QuestionInteractionPhase.ANSWERING) {
            return
        }
        val state = engine.state
        if (state.interactionPhase != QuestionInteractionPhase.CHECKED) {
            return
        }
        val question = state.currentQuestion ?: return
        if (question is RenderedQuestion.Reorder) {
            return
        }
        speakableHeadword(question)?.let(wordSpeech::speak)
    }

    companion object {
        fun factory(dependencies: AppDependencies): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    ReviewViewModel(
                        vocabularyRepository = dependencies.vocabularyRepository,
                        reviewStore = dependencies.reviewStore,
                        wordSpeech = dependencies.wordSpeech,
                        sessionUserId = dependencies::activeUserId,
                        canRefreshFromSupabase = dependencies::canRefreshFromSupabase,
                        vocabularySync = dependencies.vocabularySyncCoordinator,
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
