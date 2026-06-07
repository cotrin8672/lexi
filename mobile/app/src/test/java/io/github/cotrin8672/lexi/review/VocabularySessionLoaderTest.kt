package io.github.cotrin8672.lexi.review

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.storage.VocabularyLoadResult
import io.github.cotrin8672.lexi.review.storage.VocabularyRepository
import io.github.cotrin8672.lexi.review.storage.VocabularySource
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class VocabularySessionLoaderTest {
    @Test
    fun returnsCachedVocabularyBeforeSupabaseRefresh() = runBlocking {
        val cards = ReviewFixtures.vocabularyBundle().activeCards()
        val repository = FakeVocabularyRepository(
            cached = VocabularyLoadResult.Success(cards, VocabularySource.LOCAL_CACHE),
            refresh = VocabularyLoadResult.Success(cards, VocabularySource.SUPABASE_REFRESH),
        )

        val result = loadSessionVocabulary(
            repository = repository,
            userId = ReviewFixtures.USER_ID,
            canRefreshFromSupabase = true,
        )

        assertTrue(result is VocabularyLoadResult.Success)
        assertEquals(VocabularySource.LOCAL_CACHE, (result as VocabularyLoadResult.Success).source)
        assertEquals(0, repository.refreshCalls)
    }

    @Test
    fun refreshesFromSupabaseWhenCacheIsEmpty() = runBlocking {
        val cards = ReviewFixtures.vocabularyBundle().activeCards()
        val repository = FakeVocabularyRepository(
            cached = VocabularyLoadResult.Failure("No cached vocabulary for user"),
            refresh = VocabularyLoadResult.Success(cards, VocabularySource.SUPABASE_REFRESH),
        )

        val result = loadSessionVocabulary(
            repository = repository,
            userId = ReviewFixtures.USER_ID,
            canRefreshFromSupabase = true,
        )

        assertTrue(result is VocabularyLoadResult.Success)
        assertEquals(VocabularySource.SUPABASE_REFRESH, (result as VocabularyLoadResult.Success).source)
        assertEquals(1, repository.refreshCalls)
    }

    @Test
    fun surfacesSupabaseFailureWhenCacheIsEmpty() = runBlocking {
        val repository = FakeVocabularyRepository(
            cached = VocabularyLoadResult.Failure("No cached vocabulary for user"),
            refresh = VocabularyLoadResult.Failure("HTTP 401"),
        )

        val result = loadSessionVocabulary(
            repository = repository,
            userId = ReviewFixtures.USER_ID,
            canRefreshFromSupabase = true,
        )

        assertTrue(result is VocabularyLoadResult.Failure)
        val message = (result as VocabularyLoadResult.Failure).message
        assertTrue(message.contains("Supabase sync failed"))
        assertTrue(message.contains("HTTP 401"))
    }

    @Test
    fun failsWhenNoUserAndRefreshUnavailable() = runBlocking {
        val repository = FakeVocabularyRepository(
            cached = VocabularyLoadResult.Failure("No cached vocabulary for user"),
            refresh = VocabularyLoadResult.Failure("not configured"),
        )

        val result = loadSessionVocabulary(
            repository = repository,
            userId = null,
            canRefreshFromSupabase = false,
        )

        assertTrue(result is VocabularyLoadResult.Failure)
        assertEquals(0, repository.refreshCalls)
    }

    private class FakeVocabularyRepository(
        private val cached: VocabularyLoadResult,
        private val refresh: VocabularyLoadResult,
    ) : VocabularyRepository {
        var refreshCalls = 0

        override suspend fun loadFixtureCards(): VocabularyLoadResult =
            VocabularyLoadResult.Failure("fixture path not used")

        override suspend fun loadCachedCards(userId: String): VocabularyLoadResult = cached

        override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult {
            refreshCalls += 1
            return refresh
        }
    }
}
