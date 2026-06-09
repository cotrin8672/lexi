package io.github.cotrin8672.lexi.review.ui

import io.github.cotrin8672.lexi.review.QuestionStats
import io.github.cotrin8672.lexi.review.QuestionType
import io.github.cotrin8672.lexi.review.ReviewResult
import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.storage.RecordingStatsStore
import io.github.cotrin8672.lexi.review.storage.ReviewAttemptEvent
import io.github.cotrin8672.lexi.review.storage.ReviewStore
import io.github.cotrin8672.lexi.review.storage.StudySession
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
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
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

@OptIn(ExperimentalCoroutinesApi::class)
class StatsViewModelTest {
    private val testDispatcher = StandardTestDispatcher()
    private val zoneId = ZoneId.of("UTC")
    private val today = LocalDate.of(2026, 6, 10)
    private val now = today.atTime(18, 0).atZone(zoneId).toInstant()
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
    fun unsignedUserShowsErrorState() = runTest(testDispatcher) {
        val viewModel = StatsViewModel(
            statsStore = RecordingStatsStore(),
            vocabularyRepository = StubVocabularyRepository(),
            reviewStore = StubReviewStore(),
            sessionUserId = { null },
            now = { now },
            zoneId = zoneId,
        )

        advanceUntilIdle()

        val state = viewModel.uiState.value
        assertTrue(state is StatsUiState.Error)
        assertEquals("Sign in to view study stats.", (state as StatsUiState.Error).message)
    }

    @Test
    fun signedInUserLoadsDashboardWithTodayMetrics() = runTest(testDispatcher) {
        val statsStore = RecordingStatsStore()
        val todayMorning = instantAt(today, 10)
        val todayNoon = instantAt(today, 12)
        statsStore.seedSession(
            StudySession(
                id = "session-today",
                startedAt = todayMorning,
                endedAt = todayNoon,
                activeMillis = 180_000L,
                answeredCount = 2,
                correctCount = 1,
            ),
        )
        statsStore.seedAttempt(
            ReviewAttemptEvent(
                id = "attempt-1",
                sessionId = "session-today",
                questionKey = "key-adopt-meaning",
                questionType = QuestionType.MEANING.name,
                lexemeId = "lex-adopt",
                correct = true,
                answeredAt = todayMorning,
                elapsedActiveMillis = 30_000L,
            ),
        )
        statsStore.seedAttempt(
            ReviewAttemptEvent(
                id = "attempt-2",
                sessionId = "session-today",
                questionKey = "key-go-meaning",
                questionType = QuestionType.MEANING.name,
                lexemeId = "lex-go",
                correct = false,
                answeredAt = todayNoon,
                elapsedActiveMillis = 90_000L,
            ),
        )

        val viewModel = StatsViewModel(
            statsStore = statsStore,
            vocabularyRepository = StubVocabularyRepository(
                cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
            ),
            reviewStore = StubReviewStore(
                stats = listOf(
                    QuestionStats(
                        questionKey = "key-go-meaning",
                        questionType = QuestionType.MEANING,
                        lexemeId = "lex-go",
                        attempts = 2,
                        wrongCount = 1,
                        difficultyEma = 0.85,
                        lastResult = ReviewResult.WRONG,
                        createdAt = ReviewFixtures.FIXTURE_TIMESTAMP,
                        updatedAt = ReviewFixtures.FIXTURE_TIMESTAMP,
                    ),
                ),
            ),
            sessionUserId = { ReviewFixtures.USER_ID },
            now = { now },
            zoneId = zoneId,
        )

        advanceUntilIdle()

        val state = viewModel.uiState.value
        assertTrue(state is StatsUiState.Ready)
        val dashboard = (state as StatsUiState.Ready).dashboard
        assertEquals(3, dashboard.today.studyMinutes)
        assertEquals(2, dashboard.today.distinctLexemesReviewed)
        assertEquals(2, dashboard.today.attempts)
        assertEquals(0.5, dashboard.today.accuracy!!, 0.0001)
        assertEquals(6, dashboard.vocabularyGrowth.totalCount)
        assertEquals(1, statsStore.attemptsSinceQueries.size)
        assertEquals(1, statsStore.sessionsSinceQueries.size)
    }

    @Test
    fun refreshReloadsDashboardAfterSignIn() = runTest(testDispatcher) {
        var userId: String? = null
        val statsStore = RecordingStatsStore()
        val viewModel = StatsViewModel(
            statsStore = statsStore,
            vocabularyRepository = StubVocabularyRepository(
                cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
            ),
            reviewStore = StubReviewStore(),
            sessionUserId = { userId },
            now = { now },
            zoneId = zoneId,
        )

        advanceUntilIdle()
        assertTrue(viewModel.uiState.value is StatsUiState.Error)

        userId = ReviewFixtures.USER_ID
        viewModel.refresh()
        advanceUntilIdle()

        assertTrue(viewModel.uiState.value is StatsUiState.Ready)
    }

    private fun instantAt(day: LocalDate, hour: Int): String =
        day.atTime(hour, 0).atZone(zoneId).toInstant().toString()

    private class StubVocabularyRepository(
        private val cached: VocabularyLoadResult = VocabularyLoadResult.Failure("unused"),
    ) : VocabularyRepository {
        override suspend fun loadFixtureCards(): VocabularyLoadResult =
            VocabularyLoadResult.Failure("fixture not stubbed")

        override suspend fun loadCachedCards(userId: String): VocabularyLoadResult = cached

        override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult =
            VocabularyLoadResult.Failure("refresh not stubbed")
    }

    private class StubReviewStore(
        private val stats: List<QuestionStats> = emptyList(),
    ) : ReviewStore {
        override suspend fun getStats(questionKey: String): QuestionStats? =
            stats.firstOrNull { it.questionKey == questionKey }

        override suspend fun getAllStats(): List<QuestionStats> = stats

        override suspend fun getStatsByKeys(questionKeys: Collection<String>): Map<String, QuestionStats> =
            stats.filter { it.questionKey in questionKeys }.associateBy { it.questionKey }

        override suspend fun upsertStats(stats: QuestionStats) = Unit
    }
}
