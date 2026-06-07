package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import io.github.cotrin8672.lexi.review.ui.ReviewViewModel
import io.github.cotrin8672.lexi.review.ui.SessionLoadPhase
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
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
}
