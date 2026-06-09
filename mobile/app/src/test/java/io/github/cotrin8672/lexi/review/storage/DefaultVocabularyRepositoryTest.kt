package io.github.cotrin8672.lexi.review.storage

import io.github.cotrin8672.lexi.review.fixtures.ReviewFixtures
import io.github.cotrin8672.lexi.review.sync.VocabularyReplicaSync
import io.github.cotrin8672.lexi.review.storage.dao.VocabularyCacheDao
import io.github.cotrin8672.lexi.review.storage.entity.CachedCardSnapshotEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedLexemeFormEntity
import io.github.cotrin8672.lexi.review.storage.entity.CachedUserLexemeEntity
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DefaultVocabularyRepositoryTest {
    @Test
    fun refreshFromSupabaseReturnsFailureWhenSyncThrows() = runTest {
        val repository = DefaultVocabularyRepository(
            cacheDao = NoopVocabularyCacheDao(),
            syncEngine = VocabularyReplicaSync {
                throw IllegalStateException("Supabase request failed with HTTP 401")
            },
        )

        val result = repository.refreshFromSupabase(ReviewFixtures.USER_ID)

        assertTrue(result is VocabularyLoadResult.Failure)
        assertEquals(
            "Supabase request failed with HTTP 401",
            (result as VocabularyLoadResult.Failure).message,
        )
    }

    private class NoopVocabularyCacheDao : VocabularyCacheDao {
        override suspend fun getLexemes(userId: String): List<CachedUserLexemeEntity> = emptyList()

        override suspend fun getActiveSnapshots(userId: String): List<CachedCardSnapshotEntity> = emptyList()

        override suspend fun getForms(userId: String): List<CachedLexemeFormEntity> = emptyList()

        override suspend fun hasAppliedPullChange(
            userId: String,
            operationId: String,
            serverRevision: Long,
        ): Boolean = false

        override suspend fun deactivateSnapshots(
            userId: String,
            lexemeId: String,
            resultLanguage: String,
        ) = Unit

        override suspend fun findLexemeId(
            userId: String,
            language: String,
            canonicalKey: String,
        ): String? = null

        override suspend fun findLexemeById(
            userId: String,
            lexemeId: String,
        ): CachedUserLexemeEntity? = null

        override suspend fun findLexemeByKey(
            userId: String,
            language: String,
            canonicalKey: String,
        ): CachedUserLexemeEntity? = null

        override suspend fun upsertLexemes(rows: List<CachedUserLexemeEntity>) = Unit

        override suspend fun upsertLexeme(row: CachedUserLexemeEntity) = Unit

        override suspend fun upsertSnapshot(row: CachedCardSnapshotEntity) = Unit

        override suspend fun upsertForm(row: CachedLexemeFormEntity) = Unit

        override suspend fun upsertSnapshots(rows: List<CachedCardSnapshotEntity>) = Unit

        override suspend fun upsertForms(rows: List<CachedLexemeFormEntity>) = Unit

        override suspend fun deleteLexemes(userId: String) = Unit

        override suspend fun deleteSnapshots(userId: String) = Unit

        override suspend fun deleteForms(userId: String) = Unit

        override suspend fun replaceUserCache(
            userId: String,
            lexemes: List<CachedUserLexemeEntity>,
            snapshots: List<CachedCardSnapshotEntity>,
            forms: List<CachedLexemeFormEntity>,
        ) = Unit
    }
}
