package io.github.cotrin8672.lexi.review.sync

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class VocabularySyncCoordinatorTest {
    private val testDispatcher = StandardTestDispatcher()

    @Test
    fun probeCacheMarksLocalCacheWithoutClaimingSyncReady() = runTest(testDispatcher) {
        val repository = StubRepository(
            cached = VocabularyLoadResult.Success(
                cards = ReviewFixtures.vocabularyBundle().activeCards(),
                source = VocabularySource.LOCAL_CACHE,
            ),
        )
        val coordinator = VocabularySyncCoordinator(
            repository = repository,
            scope = CoroutineScope(SupervisorJob() + testDispatcher),
            canSync = { true },
        )

        coordinator.probeCache(ReviewFixtures.USER_ID)
        advanceUntilIdle()

        val status = coordinator.status.value
        assertTrue(status.hasLocalCache)
        assertFalse(status.cacheReady)
    }

    @Test
    fun successfulSyncMarksCacheReady() = runTest(testDispatcher) {
        val repository = StubRepository(
            cached = VocabularyLoadResult.Failure("No cached vocabulary for user"),
            refresh = VocabularyLoadResult.Success(
                cards = ReviewFixtures.vocabularyBundle().activeCards(),
                source = VocabularySource.SUPABASE_REFRESH,
            ),
        )
        val coordinator = VocabularySyncCoordinator(
            repository = repository,
            scope = CoroutineScope(SupervisorJob() + testDispatcher),
            canSync = { true },
        )

        coordinator.syncNow(ReviewFixtures.USER_ID)
        advanceUntilIdle()

        val status = coordinator.status.value
        assertTrue(status.cacheReady)
        assertTrue(status.hasLocalCache)
        assertEquals(null, status.lastError)
    }

    private class StubRepository(
        private val cached: VocabularyLoadResult,
        private val refresh: VocabularyLoadResult = VocabularyLoadResult.Failure("refresh not stubbed"),
    ) : VocabularyRepository {
        override suspend fun loadFixtureCards(): VocabularyLoadResult =
            VocabularyLoadResult.Failure("fixture not stubbed")

        override suspend fun loadCachedCards(userId: String): VocabularyLoadResult = cached

        override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult = refresh
    }
}
