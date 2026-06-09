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
import io.github.cotrin8672.lexi.review.storage.InMemoryStatsStore
import io.github.cotrin8672.lexi.review.storage.ReviewAttemptEvent
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.StatsStore
import io.github.cotrin8672.lexi.review.storage.StudySession
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.speech.NoOpWordSpeech
import io.github.cotrin8672.lexi.review.speech.WordSpeech
import io.github.cotrin8672.lexi.review.speech.speakableHeadword
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.sync.VocabularySyncCoordinator
import io.github.cotrin8672.lexi.review.sync.syncErrorUserMessage
import io.github.cotrin8672.lexi.review.stats.StudySessionTracker
import io.github.cotrin8672.lexi.review.toVocabularyListItems
import java.util.UUID
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.time.Instant

class ReviewViewModel(
    private val vocabularyRepository: VocabularyRepository,
    private val reviewStore: ReviewStore = InMemoryReviewStore(),
    private val statsStore: StatsStore = InMemoryStatsStore(),
    private val wordSpeech: WordSpeech = NoOpWordSpeech,
    private val sessionUserId: () -> String? = { null },
    private val canRefreshFromSupabase: () -> Boolean = { false },
    private val vocabularySync: VocabularySyncCoordinator? = null,
) : ViewModel() {
    private val engine = ReviewSessionEngine(
        now = { Instant.now().toString() },
    )
    private val studySessionTracker = StudySessionTracker()

    private val _uiState = MutableStateFlow(ReviewUiState())
    val uiState: StateFlow<ReviewUiState> = _uiState.asStateFlow()

    private var pendingRetryMode: ReviewMode? = null
    private var pendingVocabularyList = false
    private var pendingStatsDashboard = false
    private var lastToastedSyncError: String? = null
    private var activeStudySessionId: String? = null

    init {
        vocabularySync?.let { coordinator ->
            viewModelScope.launch {
                coordinator.status.collect { syncStatus ->
                    val shouldToast = syncStatus.lastError != null &&
                        !syncStatus.isSyncing &&
                        syncStatus.lastError != lastToastedSyncError
                    if (shouldToast) {
                        lastToastedSyncError = syncStatus.lastError
                    }
                    if (syncStatus.lastError == null) {
                        lastToastedSyncError = null
                    }
                    _uiState.update { current ->
                        current.copy(
                            vocabularySyncInProgress = syncStatus.isSyncing,
                            hasLocalCache = syncStatus.hasLocalCache,
                            vocabularyCacheReady = syncStatus.cacheReady,
                            vocabularySyncError = if (!syncStatus.isSyncing) {
                                syncStatus.lastError
                            } else {
                                null
                            },
                            syncToastMessage = if (shouldToast) {
                                syncErrorUserMessage(syncStatus.lastError!!)
                            } else {
                                current.syncToastMessage
                            },
                            syncToastNonce = if (shouldToast) {
                                current.syncToastNonce + 1
                            } else {
                                current.syncToastNonce
                            },
                        )
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

    fun syncVocabulary() {
        val userId = sessionUserId()?.takeIf { it.isNotBlank() }
        if (userId == null) {
            showSyncToast("Sign in to sync vocabulary.")
            return
        }
        if (!canRefreshFromSupabase()) {
            showSyncToast("Supabase sync is not available. Check sign-in and app configuration.")
            return
        }
        vocabularySync?.scheduleSync(userId)
    }

    fun startSession(mode: ReviewMode) {
        pendingRetryMode = mode
        pendingVocabularyList = false
        viewModelScope.launch {
            val userId = sessionUserId()?.takeIf { it.isNotBlank() }
            if (userId == null) {
                setUiState(
                    ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        reviewMode = mode,
                        errorMessage = "No account on this device. Sign in to sync vocabulary.",
                    ),
                )
                return@launch
            }
            loadAndStartSession(mode, userId)
        }
    }

    fun openStatsDashboard() {
        pendingRetryMode = null
        pendingVocabularyList = false
        pendingStatsDashboard = true
        setUiState(
            _uiState.value.copy(
                loadPhase = SessionLoadPhase.STATS_DASHBOARD,
                errorMessage = null,
            ),
        )
    }

    fun loadVocabularyList() {
        pendingVocabularyList = true
        pendingRetryMode = null
        viewModelScope.launch {
            val userId = sessionUserId()?.takeIf { it.isNotBlank() }
            if (userId == null) {
                setUiState(
                    ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        errorMessage = "No account on this device. Sign in to sync vocabulary.",
                    ),
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
                    beginStudySession()
                }
                is VocabularyLoadResult.Failure -> {
                    setUiState(
                        ReviewUiState(
                            loadPhase = SessionLoadPhase.ERROR,
                            reviewMode = ReviewMode.MIXED_RANDOM,
                            errorMessage = result.message,
                        ),
                    )
                }
            }
        }
    }

    fun retryLastLoad() {
        when {
            pendingStatsDashboard -> openStatsDashboard()
            pendingVocabularyList -> loadVocabularyList()
            pendingRetryMode != null -> startSession(pendingRetryMode!!)
            else -> loadVocabularyList()
        }
    }

    /** Called when the review screen loses foreground while a session is active. */
    fun onPause() {
        studySessionTracker.pause()
        viewModelScope.launch {
            flushSessionActiveTime()
        }
    }

    /** Called when the review screen regains foreground while a session is active. */
    fun onResume() {
        studySessionTracker.resume()
    }

    fun returnToModeSelect() {
        pendingRetryMode = null
        pendingVocabularyList = false
        pendingStatsDashboard = false
        viewModelScope.launch {
            endStudySessionIfActive()
            val syncStatus = vocabularySync?.status?.value
            _uiState.value = ReviewUiState(
                loadPhase = SessionLoadPhase.MODE_SELECT,
                vocabularySyncInProgress = syncStatus?.isSyncing == true,
                hasLocalCache = syncStatus?.hasLocalCache == true,
                vocabularyCacheReady = syncStatus?.cacheReady == true,
                vocabularySyncError = syncStatus?.lastError,
            )
        }
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
            setUiState(
                ReviewUiState(
                    loadPhase = SessionLoadPhase.SYNCING_VOCABULARY,
                    reviewMode = mode,
                ),
            )
            vocabularySync?.syncNow(resolvedUserId)
            when (val result = loadSessionVocabulary(vocabularyRepository, resolvedUserId)) {
                is VocabularyLoadResult.Success -> publishSession(mode, result)
                is VocabularyLoadResult.Failure -> {
                    setUiState(
                        ReviewUiState(
                            loadPhase = SessionLoadPhase.ERROR,
                            reviewMode = mode,
                            errorMessage = vocabularySync?.status?.value?.lastError ?: result.message,
                        ),
                    )
                }
            }
        }
    }

    fun updateInflectionAnswer(answerText: String) {
        recordStudyInteraction()
        publish(engine.updateInflectionAnswer(answerText))
    }

    fun selectOption(answerKey: String) {
        recordStudyInteraction()
        publish(engine.selectOption(answerKey))
    }

    fun submitOption(answerKey: String) {
        viewModelScope.launch {
            recordStudyInteraction()
            val beforeKey = engine.state.currentQuestion?.candidate?.questionKey
            val beforePhase = engine.state.interactionPhase
            publish(engine.submitOption(answerKey))
            speakAfterMultipleChoiceCheck(beforePhase)
            persistAnswerOutcome(beforeKey, beforePhase)
        }
    }

    fun addReorderToken(bankSlotIndex: Int) {
        recordStudyInteraction()
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
        recordStudyInteraction()
        publish(engine.removeReorderToken(selectedIndex))
    }

    fun checkAnswer() {
        viewModelScope.launch {
            recordStudyInteraction()
            val beforeKey = engine.state.currentQuestion?.candidate?.questionKey
            val beforePhase = engine.state.interactionPhase
            publish(engine.checkAnswer())
            speakAfterMultipleChoiceCheck(beforePhase)
            persistAnswerOutcome(beforeKey, beforePhase)
        }
    }

    fun skipQuestion() {
        recordStudyInteraction()
        publish(engine.skipQuestion())
    }

    fun nextQuestion() {
        recordStudyInteraction()
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
                setUiState(
                    ReviewUiState(
                        loadPhase = SessionLoadPhase.VOCABULARY_LIST,
                        vocabularySource = result.source,
                        vocabularyList = result.cards.toVocabularyListItems(),
                        vocabularyCount = result.cards.size,
                    ),
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
            setUiState(
                ReviewUiState(
                    loadPhase = SessionLoadPhase.ERROR,
                    reviewMode = mode,
                    errorMessage = cacheMessage,
                ),
            )
            return
        }

        setUiState(
            ReviewUiState(
                loadPhase = SessionLoadPhase.SYNCING_VOCABULARY,
                reviewMode = mode,
            ),
        )
        vocabularySync.syncNow(userId)

        when (val result = loadSessionVocabulary(vocabularyRepository, userId)) {
            is VocabularyLoadResult.Success -> publishSession(mode, result)
            is VocabularyLoadResult.Failure -> {
                setUiState(
                    ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        reviewMode = mode,
                        errorMessage = vocabularySync.status.value.lastError ?: result.message,
                    ),
                )
            }
        }
    }

    private suspend fun waitForSyncAndShowVocabularyList(userId: String, cacheMessage: String) {
        if (!canRefreshFromSupabase() || vocabularySync == null) {
            setUiState(
                ReviewUiState(
                    loadPhase = SessionLoadPhase.ERROR,
                    errorMessage = cacheMessage,
                ),
            )
            return
        }

        setUiState(ReviewUiState(loadPhase = SessionLoadPhase.SYNCING_VOCABULARY))
        vocabularySync.syncNow(userId)

        when (val result = loadSessionVocabulary(vocabularyRepository, userId)) {
            is VocabularyLoadResult.Success -> {
                setUiState(
                    ReviewUiState(
                        loadPhase = SessionLoadPhase.VOCABULARY_LIST,
                        vocabularySource = result.source,
                        vocabularyList = result.cards.toVocabularyListItems(),
                        vocabularyCount = result.cards.size,
                    ),
                )
            }
            is VocabularyLoadResult.Failure -> {
                setUiState(
                    ReviewUiState(
                        loadPhase = SessionLoadPhase.ERROR,
                        errorMessage = vocabularySync.status.value.lastError ?: result.message,
                    ),
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
        beginStudySession()
    }

    private suspend fun beginStudySession() {
        endStudySessionIfActive()
        val startedAt = Instant.now().toString()
        val sessionId = UUID.randomUUID().toString()
        statsStore.startSession(
            StudySession(
                id = sessionId,
                startedAt = startedAt,
                endedAt = null,
                activeMillis = 0L,
                answeredCount = 0,
                correctCount = 0,
            ),
        )
        activeStudySessionId = sessionId
        studySessionTracker.start()
    }

    private suspend fun endStudySessionIfActive() {
        val sessionId = activeStudySessionId ?: return
        val activeMillis = studySessionTracker.stop()
        statsStore.updateSessionActiveMillis(sessionId, activeMillis)
        statsStore.endSession(sessionId, Instant.now().toString())
        activeStudySessionId = null
    }

    private suspend fun flushSessionActiveTime() {
        val sessionId = activeStudySessionId ?: return
        statsStore.updateSessionActiveMillis(sessionId, studySessionTracker.currentActiveMillis())
    }

    private fun recordStudyInteraction() {
        if (activeStudySessionId == null) {
            return
        }
        studySessionTracker.recordInteraction()
    }

    private suspend fun persistAnswerOutcome(
        beforeKey: String?,
        beforePhase: QuestionInteractionPhase,
    ) {
        if (beforePhase != QuestionInteractionPhase.ANSWERING || beforeKey == null) {
            return
        }
        val afterKey = engine.state.currentQuestion?.candidate?.questionKey
        if (beforeKey != afterKey) {
            return
        }
        engine.statsSnapshot()[beforeKey]?.let { reviewStore.upsertStats(it) }
        recordAttemptEvent(beforeKey)
    }

    private suspend fun recordAttemptEvent(questionKey: String) {
        val sessionId = activeStudySessionId ?: return
        val state = engine.state
        val question = state.currentQuestion ?: return
        val correct = state.lastCheckCorrect ?: return
        if (state.interactionPhase != QuestionInteractionPhase.CHECKED &&
            state.interactionPhase != QuestionInteractionPhase.WORD_DETAIL
        ) {
            return
        }

        flushSessionActiveTime()
        val candidate = question.candidate
        statsStore.insertAttemptEvent(
            ReviewAttemptEvent(
                id = UUID.randomUUID().toString(),
                sessionId = sessionId,
                questionKey = questionKey,
                questionType = candidate.questionType.name,
                lexemeId = candidate.lexemeId,
                correct = correct,
                answeredAt = Instant.now().toString(),
                elapsedActiveMillis = studySessionTracker.currentActiveMillis(),
            ),
        )
        statsStore.incrementSessionAnswer(sessionId, correct)
    }

    private fun publish(state: ReviewUiState) {
        _uiState.update { current -> state.preserveSyncFrom(current) }
    }

    private fun setUiState(state: ReviewUiState) {
        _uiState.update { current -> state.preserveSyncFrom(current) }
    }

    private fun ReviewUiState.preserveSyncFrom(source: ReviewUiState): ReviewUiState = copy(
        vocabularySyncInProgress = source.vocabularySyncInProgress,
        hasLocalCache = source.hasLocalCache,
        vocabularyCacheReady = source.vocabularyCacheReady,
        vocabularySyncError = source.vocabularySyncError,
        syncToastMessage = source.syncToastMessage,
        syncToastNonce = source.syncToastNonce,
    )

    private fun showSyncToast(message: String) {
        _uiState.update { current ->
            current.copy(
                syncToastMessage = message,
                syncToastNonce = current.syncToastNonce + 1,
            )
        }
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
                        statsStore = dependencies.statsStore,
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
