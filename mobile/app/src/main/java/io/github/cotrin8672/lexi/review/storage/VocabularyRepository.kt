package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.schema.ActiveVocabularyCard
import io.github.cotrin8672.lexi.review.schema.VocabularyBundle
import io.github.cotrin8672.lexi.review.storage.dao.VocabularyCacheDao
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

enum class VocabularySource {
    FIXTURE,
    LOCAL_CACHE,
    SUPABASE_REFRESH,
}

sealed class VocabularyLoadResult {
    data class Success(
        val cards: List<ActiveVocabularyCard>,
        val source: VocabularySource,
    ) : VocabularyLoadResult()

    data class Failure(val message: String) : VocabularyLoadResult()
}

interface VocabularyRepository {
    suspend fun loadFixtureCards(): VocabularyLoadResult
    suspend fun loadCachedCards(userId: String): VocabularyLoadResult
    suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult
}

object FixtureVocabularyRepository : VocabularyRepository {
    override suspend fun loadFixtureCards(): VocabularyLoadResult = withContext(Dispatchers.Default) {
        VocabularyLoadResult.Success(
            cards = ReviewFixtures.vocabularyBundle().activeCards(),
            source = VocabularySource.FIXTURE,
        )
    }

    override suspend fun loadCachedCards(userId: String): VocabularyLoadResult =
        VocabularyLoadResult.Failure("Cached vocabulary is not loaded in fixture mode")

    override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult =
        VocabularyLoadResult.Failure("Supabase refresh is not configured")
}

class DefaultVocabularyRepository(
    private val cacheDao: VocabularyCacheDao,
    private val supabaseClient: SupabaseVocabularyClient,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : VocabularyRepository {
    override suspend fun loadFixtureCards(): VocabularyLoadResult = withContext(ioDispatcher) {
        VocabularyLoadResult.Success(
            cards = ReviewFixtures.vocabularyBundle().activeCards(),
            source = VocabularySource.FIXTURE,
        )
    }

    override suspend fun loadCachedCards(userId: String): VocabularyLoadResult =
        withContext(ioDispatcher) {
            val lexemes = cacheDao.getLexemes(userId)
            val snapshots = cacheDao.getActiveSnapshots(userId)
            val forms = cacheDao.getForms(userId)
            if (lexemes.isEmpty() || snapshots.isEmpty()) {
                return@withContext VocabularyLoadResult.Failure("No cached vocabulary for user")
            }
            val bundle = VocabularyBundle(
                lexemes = lexemes.map { it.toDomain() },
                snapshots = snapshots.map { it.toDomain() },
                forms = forms.map { it.toDomain() },
            )
            VocabularyLoadResult.Success(
                cards = bundle.activeCards(),
                source = VocabularySource.LOCAL_CACHE,
            )
        }

    override suspend fun refreshFromSupabase(userId: String): VocabularyLoadResult =
        withContext(ioDispatcher) {
            val remote = supabaseClient.fetchActiveVocabulary(userId)
            val updatedAt = remote.snapshots.firstOrNull()?.createdAt ?: ReviewFixtures.FIXTURE_TIMESTAMP
            val entities = remote.toCacheEntities(updatedAt)
            cacheDao.replaceUserCache(
                userId = userId,
                lexemes = entities.lexemes,
                snapshots = entities.snapshots,
                forms = entities.forms,
            )
            VocabularyLoadResult.Success(
                cards = remote.activeCards(),
                source = VocabularySource.SUPABASE_REFRESH,
            )
        }
}
