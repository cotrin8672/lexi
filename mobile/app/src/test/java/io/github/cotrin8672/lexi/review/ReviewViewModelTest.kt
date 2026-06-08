package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.speech.WordSpeech
import io.github.cotrin8672.lexi.review.speech.speakableHeadword
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.ui.QuestionInteractionPhase
import io.github.cotrin8672.lexi.review.ui.RenderedQuestion
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.sync.VocabularySyncCoordinator
import io.github.cotrin8672.lexi.review.ui.ReviewViewModel
import io.github.cotrin8672.lexi.review.ui.SessionLoadPhase
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ReviewViewModelTest {
    private val testDispatcher = StandardTestDispatcher()
    private val cards = ReviewFixtures.vocabularyBundle().activeCards()

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun startSessionUsesCachedVocabularyNotFixtures() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { ReviewFixtures.USER_ID },
            canRefreshFromSupabase = { true },
        )

        viewModel.startSession(ReviewMode.MEANING_ONLY)
        advanceUntilIdle()

        assertEquals(0, repository.fixtureCalls)
        assertEquals(1, repository.cacheCalls)
        val state = viewModel.uiState.value
        assertEquals(SessionLoadPhase.READY, state.loadPhase)
        assertEquals(VocabularySource.LOCAL_CACHE, state.vocabularySource)
        assertEquals(cards.size, state.vocabularyCount)
    }

    @Test
    fun startSessionHydratesPersistedStatsFromReviewStore() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
        )
        val candidate = extractQuestionCandidates(cards).first()
        val persistedStats = QuestionStats(
            questionKey = candidate.questionKey,
            questionType = candidate.questionType,
            lexemeId = candidate.lexemeId,
            attempts = 3,
            wrongCount = 2,
            difficultyEma = 0.88,
            lastResult = ReviewResult.WRONG,
            createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
            updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
        )
        val reviewStore = RecordingReviewStore(
            seeded = mapOf(candidate.questionKey to persistedStats),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            reviewStore = reviewStore,
            sessionUserId = { ReviewFixtures.USER_ID },
        )

        viewModel.startSession(ReviewMode.MIXED_RANDOM)
        advanceUntilIdle()

        assertEquals(SessionLoadPhase.READY, viewModel.uiState.value.loadPhase)
        assertTrue(reviewStore.statsKeysRequested.contains(candidate.questionKey))
    }

    @Test
    fun loadVocabularyListShowsWordsFromCache() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { ReviewFixtures.USER_ID },
        )

        viewModel.loadVocabularyList()
        advanceUntilIdle()

        val state = viewModel.uiState.value
        assertEquals(SessionLoadPhase.VOCABULARY_LIST, state.loadPhase)
        assertEquals(cards.size, state.vocabularyCount)
        assertEquals(VocabularySource.LOCAL_CACHE, state.vocabularySource)
        assertTrue(state.vocabularyList.any { it.headword == "adopt" })
    }

    @Test
    fun startSessionUsesCacheOnlyAndSyncsInBackground() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
            refresh = VocabularyLoadResult.Success(cards, VocabularySource.SUPABASE_REFRESH),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { ReviewFixtures.USER_ID },
            canRefreshFromSupabase = { true },
        )

        viewModel.startSession(ReviewMode.MIXED_RANDOM)
        advanceUntilIdle()

        assertEquals(SessionLoadPhase.READY, viewModel.uiState.value.loadPhase)
        assertEquals(1, repository.cacheCalls)
        assertEquals(0, repository.refreshCalls)
        assertEquals(VocabularySource.LOCAL_CACHE, viewModel.uiState.value.vocabularySource)
    }

    @Test
    fun startSessionFailsWhenCacheEmptyAndSyncUnavailable() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Failure("No cached vocabulary for user"),
            refresh = VocabularyLoadResult.Failure("Supabase vocabulary fetch failed with HTTP 401"),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { ReviewFixtures.USER_ID },
            canRefreshFromSupabase = { true },
        )

        viewModel.startSession(ReviewMode.MIXED_RANDOM)
        advanceUntilIdle()

        assertEquals(SessionLoadPhase.ERROR, viewModel.uiState.value.loadPhase)
        assertEquals("No cached vocabulary for user", viewModel.uiState.value.errorMessage)
        assertEquals(0, repository.refreshCalls)
    }

    @Test
    fun startSessionWaitsForSyncWhenCacheEmpty() = runTest(testDispatcher) {
        val repository = CacheThenReadyRepository(cards)
        val coordinator = VocabularySyncCoordinator(
            repository = repository,
            scope = CoroutineScope(SupervisorJob() + testDispatcher),
            canSync = { true },
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { ReviewFixtures.USER_ID },
            canRefreshFromSupabase = { true },
            vocabularySync = coordinator,
        )

        viewModel.startSession(ReviewMode.MIXED_RANDOM)
        advanceUntilIdle()

        assertEquals(SessionLoadPhase.READY, viewModel.uiState.value.loadPhase)
        assertEquals(VocabularySource.LOCAL_CACHE, viewModel.uiState.value.vocabularySource)
        assertEquals(1, repository.refreshCalls)
    }

    @Test
    fun startSessionStartsImmediatelyFromCacheWithoutSync() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { ReviewFixtures.USER_ID },
        )

        viewModel.startSession(ReviewMode.MIXED_RANDOM)
        advanceUntilIdle()

        assertEquals(SessionLoadPhase.READY, viewModel.uiState.value.loadPhase)
        assertEquals(1, repository.cacheCalls)
        assertEquals(0, repository.refreshCalls)
    }

    @Test
    fun submitOptionSpeaksHeadwordAfterMultipleChoiceCheck() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
        )
        val wordSpeech = RecordingWordSpeech()
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            wordSpeech = wordSpeech,
            sessionUserId = { ReviewFixtures.USER_ID },
        )

        viewModel.startSession(ReviewMode.MEANING_ONLY)
        advanceUntilIdle()

        val question = viewModel.uiState.value.currentQuestion as RenderedQuestion.Meaning
        val answerKey = question.options.first { it.isCorrect }.answerKey
        viewModel.submitOption(answerKey)
        advanceUntilIdle()
        viewModel.submitOption(answerKey)
        advanceUntilIdle()

        assertEquals(QuestionInteractionPhase.CHECKED, viewModel.uiState.value.interactionPhase)
        assertEquals(listOf(speakableHeadword(question)), wordSpeech.spoken)
    }

    @Test
    fun addReorderTokenSpeaksSelectedToken() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
        )
        val wordSpeech = RecordingWordSpeech()
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            wordSpeech = wordSpeech,
            sessionUserId = { ReviewFixtures.USER_ID },
        )

        viewModel.startSession(ReviewMode.REORDER_ONLY)
        advanceUntilIdle()

        val bank = viewModel.uiState.value.reorderBankSlots()
        val firstAvailableIndex = bank.indexOfFirst { !it.selected }
        val expectedToken = bank[firstAvailableIndex].token
        viewModel.addReorderToken(firstAvailableIndex)

        assertEquals(listOf(expectedToken), wordSpeech.spoken)
    }

    @Test
    fun tryRefreshFromSupabaseFailsWithoutUserId() = runTest(testDispatcher) {
        val repository = RecordingVocabularyRepository(
            cached = VocabularyLoadResult.Failure("unused"),
        )
        val viewModel = ReviewViewModel(
            vocabularyRepository = repository,
            sessionUserId = { null },
        )

        viewModel.tryRefreshFromSupabase()
        advanceUntilIdle()

        assertEquals(SessionLoadPhase.ERROR, viewModel.uiState.value.loadPhase)
        assertEquals("Sign in to refresh vocabulary from Supabase.", viewModel.uiState.value.errorMessage)
        assertEquals(0, repository.refreshCalls)
    }

    private class RecordingWordSpeech : WordSpeech {
        val spoken = mutableListOf<String>()

        override fun speak(text: String) {
            spoken += text
        }
    }

    private class RecordingReviewStore(
        private val seeded: Map<String, QuestionStats> = emptyMap(),
    ) : ReviewStore {
        var statsKeysRequested: List<String> = emptyList()

        override suspend fun getStats(questionKey: String): QuestionStats? = seeded[questionKey]

        override suspend fun getAllStats(): List<QuestionStats> = seeded.values.toList()

        override suspend fun getStatsByKeys(questionKeys: Collection<String>): Map<String, QuestionStats> {
            statsKeysRequested = questionKeys.toList()
            return questionKeys.mapNotNull { key -> seeded[key]?.let { key to it } }.toMap()
        }

        override suspend fun upsertStats(stats: QuestionStats) = Unit
    }

    private class RecordingVocabularyRepository(
        private val cached: VocabularyLoadResult,
        private val refresh: VocabularyLoadResult = VocabularyLoadResult.Failure("refresh not stubbed"),
        private val fixture: VocabularyLoadResult = VocabularyLoadResult.Failure("fixture not stubbed"),
    ) : VocabularyRepository {
        var cacheCalls = 0
        var refreshCalls = 0
        var fixtureCalls = 0

        override suspend fun loadFixtureCards(): VocabularyLoadResult {
            fixtureCalls += 1
            return fixture
        }

        override suspend fun loadCachedCards(userId: String): VocabularyLoadResult {
            cacheCalls += 1
            return cached
        }

        override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult {
            refreshCalls += 1
            return refresh
        }
    }

    private class CacheThenReadyRepository(
        private val cards: List<io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard>,
    ) : VocabularyRepository {
        var refreshCalls = 0
        private var cacheReady = false

        override suspend fun loadFixtureCards(): VocabularyLoadResult =
            VocabularyLoadResult.Failure("fixture not stubbed")

        override suspend fun loadCachedCards(userId: String): VocabularyLoadResult =
            if (cacheReady) {
                VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE)
            } else {
                VocabularyLoadResult.Failure("No cached vocabulary for user")
            }

        override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult {
            refreshCalls += 1
            cacheReady = true
            return VocabularyLoadResult.Success(cards, VocabularySource.SUPABASE_REFRESH)
        }
    }
}
